package app.quest.android.capture

import androidx.lifecycle.SavedStateHandle
import app.quest.android.CaptureSamples
import app.quest.android.FakeCapturePreferences
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeRepeatRules
import app.quest.android.core.CoreHolder
import app.quest.android.core.DayLabels
import app.quest.android.settings.CapturePrefs
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.MobileException

@OptIn(ExperimentalCoroutinesApi::class)
class CaptureViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()
    private val savedState = SavedStateHandle()
    private val preferences = FakeCapturePreferences(CapturePrefs(privacyAcknowledged = true))
    private val labels = DayLabels { iso -> DayValue(iso, "Label $iso") }
    private val repeatRules = FakeRepeatRules()

    private fun TestScope.viewModel(start: String? = null) = CaptureViewModel(
        openedHolder(core, backgroundScope, main.dispatcher), savedState, preferences, labels, repeatRules, FakeClock(), "de-DE", start,
    )

    /** The network call waits for the test scheduler, so "structuring" can be seen (and cancelled). */
    private fun TestScope.viewModelWithSlowNetwork(start: String?) = CaptureViewModel(
        CoreHolder(backgroundScope, StandardTestDispatcher(testScheduler), FakeClock()) { core },
        savedState, preferences, labels, repeatRules, FakeClock(), "de-DE", start,
    )

    private fun CaptureViewModel.dictate() = state.value as CaptureUiState.Dictate

    private fun CaptureViewModel.proposal() = state.value as CaptureUiState.Proposal

    private fun TestScope.structured(): CaptureViewModel {
        val vm = viewModel("Tomorrow write the offer")
        vm.structure()
        advanceUntilIdle()
        return vm
    }

    // Dictation

    @Test
    fun startsWithTheGivenText() = runTest(main.dispatcher) {
        val vm = viewModel("Shared text")
        assertEquals("Shared text", vm.dictate().text)
        assertFalse(vm.dictate().structuring)
    }

    @Test
    fun partialTextShowsLiveAndTheFinalOneIsAppended() = runTest(main.dispatcher) {
        val vm = viewModel("First line")
        vm.startListening()
        vm.onPartial("tomorrow write")
        assertEquals(DictationState(listening = true, partial = "tomorrow write"), vm.dictate().dictation)
        assertEquals("First line", vm.dictate().text)

        vm.onFinal("tomorrow write the offer")

        assertEquals("First line\ntomorrow write the offer", vm.dictate().text)
        assertFalse(vm.dictate().dictation.listening)
    }

    @Test
    fun aDictationProblemEndsListeningAndKeepsTheText() = runTest(main.dispatcher) {
        val vm = viewModel("Typed")
        vm.startListening()
        vm.onDictationProblem(DictationProblem.NoPermission)
        assertEquals(DictationState(problem = DictationProblem.NoPermission), vm.dictate().dictation)
        assertEquals("Typed", vm.dictate().text)
        vm.startListening()
        assertNull(vm.dictate().dictation.problem)
    }

    @Test
    fun aSecondTapDoesNotSendAgain() = runTest(main.dispatcher) {
        val vm = viewModelWithSlowNetwork("Typed")
        vm.structure()
        vm.structure()
        runCurrent()
        assertEquals(1, core.captures.size)
    }

    @Test
    fun aRestoredProposalIsReadOnceTheCoreOpens() = runTest(main.dispatcher) {
        structured()
        // After process death: the same saved state, the core still opening.
        val restored = viewModelWithSlowNetwork(null)
        assertNull(restored.proposal().view)
        runCurrent()
        assertEquals("Create (4)", restored.proposal().view?.applyLabel)
    }

    @Test
    fun structuringDropsTheMicHint() = runTest(main.dispatcher) {
        val vm = viewModelWithSlowNetwork("Typed")
        vm.onDictationProblem(DictationProblem.NoPermission)
        vm.structure()
        assertTrue(vm.dictate().structuring)
        assertEquals(DictationState(), vm.dictate().dictation)
    }

    @Test
    fun theTextSurvivesProcessDeath() = runTest(main.dispatcher) {
        viewModel().editText("Half typed")
        val restored = viewModel("Would be the start text")
        assertEquals("Half typed", restored.dictate().text)
    }

    // Structuring

    @Test
    fun structureSendsTheTextWithTheLanguageAndShowsTheProposal() = runTest(main.dispatcher) {
        val vm = viewModelWithSlowNetwork("Tomorrow write the offer")
        vm.structure()
        assertTrue(vm.dictate().structuring)
        // The core opens in backgroundScope, which advanceUntilIdle does not wait for.
        runCurrent()

        assertEquals(listOf("Tomorrow write the offer" to "de-DE"), core.captures)
        val proposal = vm.proposal()
        assertEquals(CaptureSamples.draft, proposal.draft)
        assertEquals("Create (4)", proposal.view?.applyLabel)
    }

    @Test
    fun blankTextIsNotSent() = runTest(main.dispatcher) {
        val vm = viewModel("  ")
        vm.structure()
        advanceUntilIdle()
        assertEquals(emptyList<Pair<String, String>>(), core.captures)
    }

    @Test
    fun thePrivacyNoteComesFirstOnce() = runTest(main.dispatcher) {
        preferences.prefs.value = CapturePrefs(privacyAcknowledged = false)
        val vm = viewModel("Text")
        vm.structure()
        advanceUntilIdle()
        assertTrue(vm.dictate().askPrivacy)
        assertEquals(emptyList<Pair<String, String>>(), core.captures)

        vm.acknowledgePrivacy()
        advanceUntilIdle()

        assertTrue(preferences.prefs.first().privacyAcknowledged)
        assertEquals(1, core.captures.size)
        assertTrue(vm.state.value is CaptureUiState.Proposal)
    }

    @Test
    fun dismissingThePrivacyNoteSendsNothing() = runTest(main.dispatcher) {
        preferences.prefs.value = CapturePrefs(privacyAcknowledged = false)
        val vm = viewModel("Text")
        vm.structure()
        advanceUntilIdle()
        vm.dismissPrivacy()
        advanceUntilIdle()
        assertFalse(vm.dictate().askPrivacy)
        assertEquals(emptyList<Pair<String, String>>(), core.captures)
    }

    @Test
    fun eachFailureStaysOnTheTextWithItsKind() = runTest(main.dispatcher) {
        val failures = listOf(
            CaptureFailure.SignedOut,
            CaptureFailure.Offline("could not reach your Quest server (timed out)."),
            CaptureFailure.Unavailable,
            CaptureFailure.RateLimited,
            CaptureFailure.InvalidResponse,
            CaptureFailure.TextTooLong("the text is too long for AI capture."),
        )
        val vm = viewModel("Text")
        failures.forEach { failure ->
            core.captureFailure = failure
            vm.structure()
            advanceUntilIdle()
            assertEquals(failure, vm.dictate().failure)
            assertEquals("Text", vm.dictate().text)
            assertFalse(vm.dictate().structuring)
        }
    }

    @Test
    fun notConfiguredIsShownAsSuch() = runTest(main.dispatcher) {
        core.captureFailure = CaptureFailure.NotConfigured
        val vm = viewModel("Text")
        vm.structure()
        advanceUntilIdle()
        assertEquals(CaptureFailure.NotConfigured, vm.dictate().failure)
    }

    @Test
    fun anotherCoreErrorShowsItsMessage() = runTest(main.dispatcher) {
        val vm = viewModel("Text")
        core.failNext = MobileException.Internal("internal error: boom.")
        vm.structure()
        advanceUntilIdle()
        assertEquals(CaptureFailure.Other("internal error: boom."), vm.dictate().failure)
    }

    @Test
    fun editingClearsTheFailure() = runTest(main.dispatcher) {
        core.captureFailure = CaptureFailure.RateLimited
        val vm = viewModel("Text")
        vm.structure()
        advanceUntilIdle()
        vm.editText("Text, shorter")
        assertNull(vm.dictate().failure)
    }

    @Test
    fun cancelReturnsToTheTextAndDropsTheAnswer() = runTest(main.dispatcher) {
        val vm = viewModelWithSlowNetwork("Text")
        vm.structure()
        assertTrue(vm.dictate().structuring)
        vm.cancelStructuring()
        advanceUntilIdle()
        assertFalse(vm.dictate().structuring)
        assertEquals("Text", vm.dictate().text)
    }

    // Proposal

    @Test
    fun selectionChangesTheCountAndANewQuestsTasks() = runTest(main.dispatcher) {
        val vm = structured()
        vm.setWorkSelected("w1", false)
        assertEquals("Create (3)", vm.proposal().view?.applyLabel)

        vm.setQuestSelected("q1", false)

        val view = vm.proposal().view!!
        assertNull(view.tasks.first { it.key == "t2" }.quest)
        assertEquals(2, view.selectedCount)
    }

    @Test
    fun editsChangeOnlyDraftFields() = runTest(main.dispatcher) {
        val vm = structured()
        vm.editTaskTitle("t1", "Offer\nfor Miller")
        vm.setTaskQuest("t2", DraftQuestRef.Existing("q-relaunch"))
        vm.setTaskDay("t1", "2026-11-30")
        vm.setTaskBlockStart("t1", 600)
        vm.setTaskBlockEnd("t1", 0)
        vm.editQuestTitle("q1", "Taxes 2025")
        vm.editWorkTitle("w1", "Concept")
        vm.setWorkQuest("w1", null)
        vm.setWorkDay("w1", "2026-11-23")
        vm.setWorkStart("w1", 480)
        vm.setWorkEnd("w1", 0)

        val draft = vm.proposal().draft
        val t1 = draft.tasks[0]
        assertEquals("Offer for Miller", t1.title)
        assertEquals(DraftDay("2026-11-30", "Label 2026-11-30"), t1.day)
        // Picker values go in unchanged; the core reads an end of 0 as midnight.
        assertEquals(DraftBlock(600, 0), t1.block)
        val t2 = draft.tasks[1]
        assertEquals(DraftQuestRef.Existing("q-relaunch"), t2.quest)
        assertFalse(t2.questUncertain)
        assertEquals("Taxes 2025", draft.quests[0].title)
        val w1 = draft.work[0]
        assertEquals(DraftWorkTask.New("Concept", null), w1.task)
        assertEquals(DraftDay("2026-11-23", "Label 2026-11-23"), w1.day)
        assertEquals(480 to 0, w1.startMinutes to w1.endMinutes)
    }

    @Test
    fun somedayAndNoBlock() = runTest(main.dispatcher) {
        val vm = structured()
        vm.setTaskDay("t1", null)
        vm.removeTaskBlock("t1")
        val t1 = vm.proposal().draft.tasks[0]
        assertNull(t1.day)
        assertNull(t1.block)
    }

    @Test
    fun aRejectedDayShowsTheCoresReason() = runTest(main.dispatcher) {
        val vm = CaptureViewModel(
            openedHolder(core, backgroundScope, main.dispatcher), savedState, preferences,
            DayLabels { throw MobileException.InvalidDate("not a day.") }, repeatRules, FakeClock(), "en-US", "Text",
        )
        vm.structure()
        advanceUntilIdle()
        vm.setTaskDay("t1", "2026-13-01")
        assertEquals("not a day.", vm.proposal().error)
        assertEquals(CaptureSamples.draft.tasks[0].day, vm.proposal().draft.tasks[0].day)
    }

    @Test
    fun anExistingTasksTitleStays() = runTest(main.dispatcher) {
        core.captureDraft = CaptureSamples.draft.copy(work = CaptureSamples.draft.work.map { it.copy(task = DraftWorkTask.Existing("t9", "Concept")) })
        val vm = structured()
        vm.editWorkTitle("w1", "Other")
        assertEquals(DraftWorkTask.Existing("t9", "Concept"), vm.proposal().draft.work[0].task)
    }

    @Test
    fun theRepeatEditorWritesBackOrRemoves() = runTest(main.dispatcher) {
        val vm = structured()
        vm.openRepeat("t2")
        assertEquals(CaptureSamples.weekly, vm.proposal().repeat?.form)
        vm.editRepeat { it.copy(every = "2") }
        vm.saveRepeat()
        assertNull(vm.proposal().repeat)
        assertEquals("2", vm.proposal().draft.tasks[1].repeat?.every)

        vm.openRepeat("t2")
        vm.removeRepeat()
        assertNull(vm.proposal().draft.tasks[1].repeat)
    }

    @Test
    fun aRuleTheCoreRejectsStaysOpen() = runTest(main.dispatcher) {
        val vm = structured()
        vm.openRepeat("t2")
        repeatRules.error = "Every must be 1 to 999."
        vm.saveRepeat()
        assertEquals("Every must be 1 to 999.", vm.proposal().repeat?.error)
        assertEquals(CaptureSamples.weekly, vm.proposal().draft.tasks[1].repeat)
    }

    @Test
    fun theDraftSurvivesProcessDeath() = runTest(main.dispatcher) {
        val vm = structured()
        vm.setTaskSelected("t1", false)
        vm.toggleExpanded("t2")

        val restored = viewModel()

        val proposal = restored.proposal()
        assertFalse(proposal.draft.tasks[0].selected)
        assertEquals("t2", proposal.expanded)
        assertEquals(1, core.captures.size)
    }

    @Test
    fun backDropsTheProposalAndKeepsTheText() = runTest(main.dispatcher) {
        val vm = structured()
        vm.backToText()
        assertEquals("Tomorrow write the offer", vm.dictate().text)
    }

    @Test
    fun createAppliesTheEditedDraftAndShowsTheMessage() = runTest(main.dispatcher) {
        val vm = structured()
        vm.setWorkSelected("w1", false)
        vm.apply()

        assertEquals(listOf(CaptureSamples.draft.withWork("w1") { it.copy(selected = false) }), core.appliedDrafts)
        assertEquals(CaptureUiState.Applied("Created 1 quest, 1 task and 1 session", "Undo", 6_000), vm.state.value)
        // A later open (same saved state) starts fresh.
        assertEquals("", viewModel().dictate().text)
    }

    @Test
    fun undoAfterCreateTakesItBackAndCloses() = runTest(main.dispatcher) {
        val vm = structured()
        vm.apply()
        vm.undoApplied()
        assertEquals(listOf("undo-capture"), core.calls.filter { it == "undo-capture" })
        assertEquals(CaptureUiState.Closed, vm.state.value)
        vm.undoApplied()
        assertEquals("only once", 1, core.calls.count { it == "undo-capture" })
    }

    @Test
    fun undoBeforeCreateDoesNothing() = runTest(main.dispatcher) {
        val vm = structured()
        vm.undoApplied()
        assertEquals(emptyList<String>(), core.calls.filter { it == "undo-capture" })
    }

    @Test
    fun aFailedUndoSaysWhy() = runTest(main.dispatcher) {
        val vm = structured()
        vm.apply()
        core.failNext = MobileException.Rejected("the store is closed.")
        vm.undoApplied()
        assertEquals(CaptureUiState.UndoFailed("the store is closed."), vm.state.value)
    }

    @Test
    fun aFailedCreateKeepsTheProposalWithTheReason() = runTest(main.dispatcher) {
        val vm = structured()
        core.failNext = MobileException.Rejected("the quest no longer exists.")
        vm.apply()
        assertEquals("the quest no longer exists.", vm.proposal().error)
        vm.setTaskSelected("t1", false)
        assertNull(vm.proposal().error)
    }
}
