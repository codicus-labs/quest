package app.quest.android.ui.archive

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
import uniffi.quest_mobile.TrashKind

class ArchiveViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel() = ArchiveViewModel(openedHolder(core, backgroundScope, main.dispatcher))

    @Test
    fun readsTheArchiveOnlyWhileShown() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(ArchiveUiState.Loading, vm.state.value)

        vm.setShown(true)
        assertEquals(ArchiveUiState.Ready(QuestSamples.archive), vm.state.value)

        vm.setShown(false)
        assertEquals(ArchiveUiState.Loading, vm.state.value)
    }

    @Test
    fun restoresAnItemAndShowsWhatTheArchiveIsNow() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        core.archive = QuestSamples.emptyArchive

        vm.restore(TrashKind.QUEST, "q9")

        assertEquals(listOf("restore QUEST q9"), core.calls)
        assertEquals(ArchiveUiState.Ready(QuestSamples.emptyArchive), vm.state.value)
    }

    @Test
    fun aFailedRestoreShowsTheCoresReason() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        core.failNext = MobileException.Rejected("Already restored.")

        vm.restore(TrashKind.TASK, "t9")

        assertEquals("Already restored.", vm.message.value)
    }

    @Test
    fun unarchivingTakesTheQuestOutOfTheArchiveAndMakesItActive() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        vm.unarchive("q8")

        assertEquals(listOf("unarchive-quest q8"), core.calls)
        assertEquals(ArchiveUiState.Ready(QuestSamples.archive.copy(archivedQuests = emptyList())), vm.state.value)
        assertEquals("Old blog", core.quests.active.last().title)
        assertEquals("Active · 3", core.quests.activeLabel)
    }
}
