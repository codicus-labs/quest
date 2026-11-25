package app.quest.android.ui.focus

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeCore
import app.quest.android.Samples
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.FocusView
import uniffi.quest_mobile.MobileException

class FocusViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        FocusViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved)

    private val FocusViewModel.session get() = state.value as FocusUiState.Session

    @Test
    fun hiddenFocusReadsNothing() = runTest(main.dispatcher) {
        val vm = viewModel()

        assertEquals(FocusUiState.Hidden, vm.state.value)
        assertTrue(core.focusTasks.isEmpty())
    }

    @Test
    fun opensOnTheCoresSessionWithHomesQuests() = runTest(main.dispatcher) {
        val vm = viewModel()

        vm.open()

        assertEquals(FocusUiState.Session(Samples.focusSession, Samples.focusSession.title, Samples.focusSession.description, Samples.home.quests), vm.state.value)
        assertEquals(listOf<String?>(null), core.focusTasks)
    }

    @Test
    fun theFocusTaskGoesToTheCoreAndSurvivesProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        viewModel(saved).rememberTask("t4")

        val restored = viewModel(saved)
        restored.open()

        assertEquals("t4", core.focusTasks.last())
    }

    @Test
    fun nothingToFocusOnShowsThePickList() = runTest(main.dispatcher) {
        core.focus = Samples.pick
        val vm = viewModel()
        vm.open()

        assertEquals(FocusUiState.Pick(Samples.pick.tasks), vm.state.value)

        vm.pick("t4")
        assertEquals(listOf("start t4"), core.calls)
        assertEquals("t4", core.focusTasks.last())
    }

    @Test
    fun pauseAndResumeUseTheTimer() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()

        vm.pause()
        core.focus = FocusView.Session(Samples.pausedSession)
        vm.resume()

        assertEquals(listOf("stop", "start t1"), core.calls)
    }

    @Test
    fun completeSavesDraftsCompletesAndGoesHome() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()
        vm.editDescription("Three scenes.")

        vm.complete()

        assertEquals(listOf("describe t1 Three scenes.", "complete t1"), core.calls)
        assertEquals(FocusUiState.Hidden, vm.state.value)
        assertEquals(FocusUiState.Hidden, vm.state.value)
    }

    @Test
    fun aFailedCompleteStaysWithTheMessage() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()
        core.failNext = MobileException.Internal("boom")

        vm.complete()

        assertTrue(vm.state.value != FocusUiState.Hidden)
        assertEquals("boom", vm.message.value)
    }

    @Test
    fun titleDraftShowsUntilCommittedAndEmptyTitlesAreTheCoresCall() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()

        vm.editTitle("Outline\nchapter three")
        assertEquals("Outline chapter three", vm.session.title)

        vm.commitTitle()
        assertEquals(listOf("rename t1 Outline chapter three"), core.calls)
        assertEquals(Samples.focusSession.title, vm.session.title)
    }

    @Test
    fun draftsOfAnotherTaskAreNotShownButSavedOnLeaving() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()
        vm.editDescription("Notes for t1")

        core.focus = FocusView.Session(Samples.focusSession.copy(taskId = "t4", title = "Edit chapter 2", description = ""))
        vm.rememberTask("t4")
        assertEquals("", vm.session.description)

        vm.close()
        assertEquals(listOf("describe t1 Notes for t1"), core.calls)
        assertEquals(FocusUiState.Hidden, vm.state.value)
    }

    @Test
    fun theQuestChipSetsTheTasksQuest() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()

        vm.setQuest("q2")
        vm.setQuest(null)

        assertEquals(listOf("quest t1 q2", "quest t1 null"), core.calls)
        assertNull(vm.message.value)
    }

    @Test
    fun resumeAndPickReportWhetherTheTimerStarted() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()

        assertTrue(vm.resume())
        core.failNext = MobileException.Internal("boom")
        assertFalse(vm.pick("t4"))

        assertEquals(listOf("start t1"), core.calls)
        assertEquals("boom", vm.message.value)
    }

    @Test
    fun draftsAreSavedWhenTheAppGoesToTheBackground() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()
        vm.editDescription("Half typed")

        vm.commitDrafts()

        assertEquals(listOf("describe t1 Half typed"), core.calls)
        assertTrue(vm.state.value != FocusUiState.Hidden)
    }
}
