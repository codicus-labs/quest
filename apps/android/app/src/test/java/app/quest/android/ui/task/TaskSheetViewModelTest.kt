package app.quest.android.ui.task

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeRepeatRules
import app.quest.android.Samples
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.RepeatFrequency
import uniffi.quest_mobile.RepeatWeekday

class TaskSheetViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()
    private val rules = FakeRepeatRules()

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        TaskSheetViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved, rules, FakeClock())

    private val TaskSheetViewModel.open get() = state.value as TaskSheetState.Open

    @Test
    fun opensWithTheCoresDetailAndClosesAgain() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(TaskSheetState.Closed, vm.state.value)

        vm.open("t1")
        assertEquals(TaskSheetState.Open(Samples.detail, Samples.detail.title, Samples.detail.description, SheetPage.Details, null), vm.state.value)

        vm.close()
        assertEquals(TaskSheetState.Closed, vm.state.value)
    }

    @Test
    fun typedTitleAndDescriptionAreSavedOnCommitOrClose() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("t1")

        vm.editTitle("Outline chapter three")
        assertEquals("Outline chapter three", vm.open.title)
        vm.commitTitle()
        vm.editDescription("Two scenes.")
        vm.close()

        assertEquals(listOf("rename t1 Outline chapter three", "describe t1 Two scenes."), core.calls)
    }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test
    fun noteSavesAfter300MillisecondsWithoutTyping() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("t1")
        vm.editDescription("First draft")
        advanceTimeBy(250)
        vm.editDescription("Final draft")
        advanceTimeBy(299)
        assertEquals(emptyList<String>(), core.calls)

        advanceTimeBy(1)
        runCurrent()
        assertEquals(listOf("describe t1 Final draft"), core.calls)
    }

    @Test
    fun questDateDoneAndDeleteGoToTheCore() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("t1")

        vm.setQuest(null)
        vm.setDay("2026-11-24")
        vm.setDay(null)
        vm.toggleDone()
        vm.delete()

        assertEquals(listOf("quest t1 null", "day t1 2026-11-24", "day t1 null", "delete t1"), core.calls)
        assertEquals(listOf("t1"), core.toggled)
        assertEquals(TaskSheetState.Closed, vm.state.value)
    }

    @Test
    fun aTaskThatIsGoneClosesTheSheet() = runTest(main.dispatcher) {
        val vm = viewModel()

        vm.open("t9")

        assertEquals(TaskSheetState.Closed, vm.state.value)
    }

    @Test
    fun repeatPageStartsFromTheCoresDraftAndShowsItsCheck() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("t1")

        vm.openRepeat()
        vm.editRepeat { it.copy(every = "3") }
        vm.toggleWeekday(RepeatWeekday.THU)

        val repeat = vm.open.repeat!!
        assertEquals(SheetPage.Repeat, vm.open.page)
        assertEquals("3", repeat.form.every)
        assertEquals(listOf(RepeatWeekday.MON), repeat.check.shownWeekdays)
    }

    @Test
    fun savingARepeatTheCoreRejectsKeepsThePageWithItsMessage() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("t1")
        vm.openRepeat()
        core.failNext = MobileException.InvalidRepeat("Enter an interval from 1 to 999.")

        vm.saveRepeat()
        assertEquals("Enter an interval from 1 to 999.", vm.open.repeat?.error)

        vm.editRepeat { it.copy(every = "1") }
        assertNull(vm.open.repeat?.error)
        vm.saveRepeat()

        assertEquals(listOf("repeat t1 1 WEEK"), core.calls)
        assertEquals(SheetPage.Details, vm.open.page)
    }

    @Test
    fun liveCheckErrorShowsWhileEditing() = runTest(main.dispatcher) {
        rules.error = "Enter a start date."
        val vm = viewModel()
        vm.open("t1")

        vm.openRepeat()

        assertEquals("Enter a start date.", vm.open.repeat?.error)
    }

    @Test
    fun stopRepeatingClearsTheSeriesRule() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("t1")
        vm.openRepeat()

        vm.stopRepeating()

        assertEquals(listOf("norepeat t1"), core.calls)
        assertEquals(SheetPage.Details, vm.open.page)
    }

    @Test
    fun drafsAndRepeatFormSurviveProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val first = viewModel(saved)
        first.open("t1")
        first.editDescription("Half typed")
        first.openRepeat()
        first.editRepeat { it.copy(frequency = RepeatFrequency.MONTH, lastDay = true) }

        val restored = viewModel(SavedStateHandle(saved.keys().associateWith { saved.get<Any>(it) }))

        assertEquals("Half typed", restored.open.description)
        assertEquals(RepeatFrequency.MONTH, restored.open.repeat?.form?.frequency)
        assertEquals(true, restored.open.repeat?.form?.lastDay)
    }

    @Test
    fun aFailedReadKeepsTheSheetAndItsDraftsAndShowsTheMessage() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("t1")
        vm.editTitle("Outline chapter three")

        core.failNext = MobileException.Internal("internal error: boom. Please report this.")
        vm.editDescription("Half typed")

        assertEquals(Samples.detail, vm.open.detail)
        assertEquals("Outline chapter three", vm.open.title)
        assertEquals("Half typed", vm.open.description)
        assertEquals("internal error: boom. Please report this.", vm.message.value)
    }

    @Test
    fun aTaskDeletedElsewhereClosesTheSheet() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("t1")

        core.details.remove("t1")
        vm.editTitle("Too late")

        assertEquals(TaskSheetState.Closed, vm.state.value)
    }
}
