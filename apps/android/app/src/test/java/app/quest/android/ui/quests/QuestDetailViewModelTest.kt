package app.quest.android.ui.quests

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeCore
import app.quest.android.FakeHomePreferences
import app.quest.android.QuestSamples
import app.quest.android.settings.HomePrefs
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.QuestHue

class QuestDetailViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        QuestDetailViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved, FakeHomePreferences(HomePrefs(addPosition = AddPosition.TOP)))

    private val QuestDetailViewModel.ready get() = state.value as QuestDetailUiState.Ready

    @Test
    fun hiddenUntilOpenedThenTheQuest() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(QuestDetailUiState.Hidden, vm.state.value)

        vm.open("q1")

        assertEquals(QuestSamples.detail, vm.ready.detail)
    }

    @Test
    fun theTitleSavesOnClose() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("q1")
        vm.editTitle("The book")
        assertEquals("The book", vm.ready.title)

        vm.close()

        assertEquals(listOf("rename-quest q1 The book"), core.calls)
        assertEquals(QuestDetailUiState.Hidden, vm.state.value)
    }

    @Test
    fun renamingTurnsTheNameIntoAFieldUntilSaved() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("q1")
        assertEquals(false, vm.ready.renamingTitle)

        vm.beginRename()
        assertEquals(true, vm.ready.renamingTitle)
        assertEquals(QuestSamples.detail.title, vm.ready.title)
        vm.editTitle("The book")
        vm.beginRename()
        assertEquals("The book", vm.ready.title)
        vm.commitTitle()

        assertEquals(false, vm.ready.renamingTitle)
        assertEquals(listOf("rename-quest q1 The book"), core.calls)
    }

    @Test
    fun thePageAndItsDraftsSurviveProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.open("q1")
        vm.editTaskDraft("Write the epilogue")
        vm.setToBacklog(true)

        val restored = viewModel(saved)

        assertEquals("Write the epilogue", restored.ready.inputs.taskDraft)
        assertEquals(true, restored.ready.inputs.toBacklog)
    }

    @Test
    fun colourAndStatusGoToTheCore() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("q1")

        vm.setHue(QuestHue.TEAL)
        vm.toggleStatus()
        assertEquals("Completed", vm.ready.detail.badge)
        assertEquals("Reopen quest", vm.ready.detail.statusAction)
        vm.undo()

        assertEquals(QuestSamples.detail, vm.ready.detail)
        assertEquals(listOf("hue q1 TEAL", "toggle-quest q1", "undo"), core.calls)
    }

    @Test
    fun addsTasksTodayOrToTheBacklogAtHomesPosition() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("q1")
        vm.editTaskDraft("Write the epilogue")
        vm.addTask()
        vm.setToBacklog(true)
        vm.editTaskDraft("Find an agent")
        vm.addTask()

        assertEquals(listOf("quest-add q1 Write the epilogue today TOP", "quest-add q1 Find an agent backlog TOP"), core.calls)
        assertEquals("", vm.ready.inputs.taskDraft)
    }

    @Test
    fun deletingClosesThePageWithoutSaving() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("q1")
        vm.editTitle("Unsaved")

        vm.delete()

        assertEquals(listOf("delete-quest q1"), core.calls)
        assertEquals(QuestDetailUiState.Hidden, vm.state.value)
    }

    @Test
    fun aQuestGoneElsewhereClosesThePage() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("q1")
        vm.editTitle("Unsaved")

        core.questDetails.remove("q1")
        vm.toggleTask("t1")

        assertEquals(QuestDetailUiState.Hidden, vm.state.value)
        assertEquals(emptyList<String>(), core.calls)
    }
}
