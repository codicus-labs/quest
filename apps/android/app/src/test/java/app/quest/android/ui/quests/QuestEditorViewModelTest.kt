package app.quest.android.ui.quests

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeCore
import app.quest.android.QuestSamples
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.QuestHue

class QuestEditorViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    /** The holder with the Quests tab shown, as when the editor opens. */
    private fun TestScope.holder() = openedHolder(core, backgroundScope, main.dispatcher).also { it.setQuestsShown(true) }

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) = QuestEditorViewModel(holder(), saved)

    private val QuestEditorViewModel.editing get() = state.value as QuestEditorState.Editing
    private val QuestEditorViewModel.new get() = state.value as QuestEditorState.New

    @Test
    fun aNewQuestIsCreatedFromItsTitleAndColourThenEdited() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.openNew()
        assertEquals(QuestEditorState.New("", QuestSamples.quests.newQuestHues, null), vm.state.value)

        vm.editTitle("Learn\npiano")
        vm.setHue(QuestHue.PINK)
        assertEquals(QuestHue.PINK, vm.new.hues.single { it.selected }.hue)
        core.questDetails["q-new"] = QuestSamples.detail.copy(id = "q-new", title = "Learn piano")
        vm.create()

        assertEquals(listOf("create-quest Learn piano|PINK"), core.calls)
        assertEquals("q-new", vm.editing.detail.id)
    }

    @Test
    fun createWithoutATitleShowsTheCoresReasonUntilTyping() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.openNew()

        vm.titleDone()
        assertEquals("Enter a name.", vm.new.error)
        assertEquals(emptyList<String>(), core.calls)

        vm.editTitle("L")
        assertEquals(null, vm.new.error)
    }

    @Test
    fun closingANewQuestWithoutCreateStoresNothing() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.openNew()
        vm.editTitle("Learn piano")

        vm.close()

        assertEquals(QuestEditorState.Closed, vm.state.value)
        assertEquals(emptyList<String>(), core.calls)
    }

    @Test
    fun theTitleSavesOnCommitAndOnClose() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("q1")
        assertEquals(QuestSamples.detail, vm.editing.detail)
        vm.editTitle("The book")
        vm.titleDone()
        vm.setHue(QuestHue.TEAL)
        vm.editTitle("The novel")

        vm.close()

        assertEquals(listOf("rename-quest q1 The book", "hue q1 TEAL", "rename-quest q1 The novel"), core.calls)
        assertEquals(QuestEditorState.Closed, vm.state.value)
    }

    @Test
    fun completingAndReopeningMovesTheQuestWithItsHoursAndUndoTakesItBack() = runTest(main.dispatcher) {
        val holder = holder()
        val vm = QuestEditorViewModel(holder, SavedStateHandle())
        val card = QuestSamples.writingCard

        vm.open("q1")
        vm.toggleStatus()
        assertEquals(QuestEditorState.Closed, vm.state.value)
        assertEquals(listOf(QuestSamples.fitnessCard), core.quests.active)
        assertEquals(card, core.quests.completed.first())
        assertEquals("Active · 1" to "Completed · 2", core.quests.activeLabel to core.quests.completedLabel)
        assertEquals("Completed quest “Write the book”", core.quests.toast?.text)

        vm.open("q1")
        assertEquals("Reopen quest", vm.editing.detail.statusAction)
        vm.toggleStatus()
        assertEquals(card, core.quests.active.last())
        assertEquals("Active · 2" to "Completed · 1", core.quests.activeLabel to core.quests.completedLabel)

        holder.perform { c, now -> c.undo(now) }
        assertEquals(card, core.quests.completed.first())
        assertEquals(listOf("toggle-quest q1", "toggle-quest q1", "undo"), core.calls)
    }

    @Test
    fun deletingSavesTheDraftsFirstThenClosesTheSheet() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("q1")
        vm.editTitle("Typed")

        vm.delete()

        assertEquals(listOf("rename-quest q1 Typed", "delete-quest q1"), core.calls)
        assertEquals(QuestEditorState.Closed, vm.state.value)
    }

    @Test
    fun openQuestPageSavesClosesAndNamesTheQuest() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("q1")
        vm.editTitle("Draft")

        assertEquals("q1", vm.openPage())
        assertEquals(listOf("rename-quest q1 Draft"), core.calls)
        assertEquals(QuestEditorState.Closed, vm.state.value)
    }

    @Test
    fun theSheetAndItsTypingSurviveProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.open("q1")
        vm.editTitle("Draft")

        val restored = viewModel(saved)

        assertEquals("Draft", restored.editing.title)
    }

    @Test
    fun aFailedReadAfterProcessDeathKeepsTheSheetAndItsTyping() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val holder = holder()
        QuestEditorViewModel(holder, saved).apply {
            open("q1")
            editTitle("Draft")
        }

        core.questDetailFailure = MobileException.ServerUnreachable("could not reach your Quest server (timed out).")
        val restored = QuestEditorViewModel(holder, saved)
        assertEquals(QuestEditorState.Closed, restored.state.value)

        core.questDetailFailure = null
        restored.editTitle("Draft again")
        assertEquals("Draft again", restored.editing.title)
    }

    @Test
    fun aQuestDeletedElsewhereClosesItsSheet() = runTest(main.dispatcher) {
        val holder = holder()
        val vm = QuestEditorViewModel(holder, SavedStateHandle())
        vm.open("q1")

        core.questDetails.remove("q1")
        core.quests = QuestSamples.listed(core.quests, listOf(QuestSamples.fitnessCard), core.quests.completed)
        holder.perform { c, now -> c.undo(now) }

        assertEquals(QuestEditorState.Closed, vm.state.value)
    }
}
