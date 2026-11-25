package app.quest.android.ui.plan

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeHomePreferences
import app.quest.android.FakePlanRules
import app.quest.android.PlanSamples
import app.quest.android.settings.HomePrefs
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.PlanWindow

class PlanViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()
    private val rules = FakePlanRules()
    private val preferences = FakeHomePreferences(HomePrefs(addPosition = AddPosition.BOTTOM))

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        PlanViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved, preferences, rules, FakeClock())

    private val PlanViewModel.ready get() = state.value as PlanUiState.Ready

    @Test
    fun buildsNothingUntilShownThenTheWindowAroundToday() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(PlanUiState.Loading, vm.state.value)
        assertEquals(emptyList<Any>(), core.planReads)

        vm.setShown(true)

        assertEquals(PlanSamples.plan, vm.ready.plan)
        assertEquals(listOf(PlanWindow("2026-11-23", 7u) to null), core.planReads.distinct())
        assertEquals(ScrollRequest(null, 0, animate = false), vm.ready.scroll)
    }

    @Test
    fun keepsTheWindowWhileItCoversWhatShowsAndBuildsAnotherOtherwise() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        vm.onVisibleRange("2026-11-24", 3)
        assertEquals(setOf(PlanWindow("2026-11-23", 7u)), core.planReads.map { it.first }.toSet())

        vm.onVisibleRange("2026-11-20", 3)
        assertEquals(PlanWindow("2026-11-20", 7u), core.planReads.last().first)
    }

    @Test
    fun leavingThePlanStopsBuildingAndLeavesItInTheCore() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        vm.setShown(false)

        assertEquals(listOf("leave-plan"), core.calls)
        assertEquals(PlanUiState.Loading, vm.state.value)
    }

    @Test
    fun comingBackToThePlanScrollsToTheDayShownBefore() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        vm.onVisibleRange("2026-11-25", 2)
        vm.scrolled(vm.ready.scroll!!)

        vm.setShown(false)
        vm.setShown(true)

        assertEquals(ScrollRequest("2026-11-25", 1, animate = false), vm.ready.scroll)
    }

    @Test
    fun comesBackToTheDayShownBeforeProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        viewModel(saved).onVisibleRange("2026-11-25", 2)

        val restored = viewModel(saved)
        restored.setShown(true)

        assertEquals("2026-11-25", restored.ready.scroll?.day)
        assertEquals(PlanWindow("2026-11-25", 7u), core.planReads.last().first)
    }

    @Test
    fun todayAndAPickedDateBuildAFreshWindowAndScrollThere() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        vm.onVisibleRange("2026-12-20", 3)

        vm.jumpTo(null)
        val request = vm.ready.scroll!!
        assertEquals(ScrollRequest(null, 1, animate = true), request)
        assertEquals(PlanWindow("2026-11-23", 7u), core.planReads.last().first)

        vm.scrolled(request)
        assertNull(vm.ready.scroll)
    }

    @Test
    fun theQuestFilterGoesToTheCoresQuery() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        vm.setQuestFilter("q1")

        assertEquals("q1", core.planReads.last().second)
        assertEquals("q1", vm.ready.inputs.questFilter)
    }

    @Test
    fun addsADaysDraftAtTheSavedPositionAndClearsIt() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        vm.editDraft("2026-11-24", "Call the printer")
        vm.editDraft("2026-11-25", "   ")

        vm.addTask("2026-11-24")
        vm.addTask("2026-11-25")

        assertEquals(listOf("plan-add 2026-11-24 Call the printer BOTTOM"), core.calls)
        assertEquals(mapOf("2026-11-25" to "   "), vm.ready.inputs.drafts)
    }

    @Test
    fun movesGoToTheCore() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        vm.moveTask("t1", "2026-11-24", "p2")
        vm.moveTask("t4", "2026-11-25", null)
        vm.nudgeTask("t4", up = true)
        vm.shiftTask("t3", later = true)

        assertEquals(
            listOf("plan-move t1 2026-11-24 p2", "plan-move t4 2026-11-25 null", "plan-nudge t4 up", "plan-shift t3 later"),
            core.calls,
        )
    }

    @Test
    fun moveToOpensTheCoresTargetsAndMovesToTheEndOfTheChosenDay() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        vm.openMoveTo("p1", "Sketch the contact sheet")
        assertEquals(MoveSheet("p1", "Sketch the contact sheet", PlanSamples.targets), vm.ready.moveSheet)

        vm.moveTo(null)

        assertNull(vm.ready.moveSheet)
        assertEquals(listOf("plan-move p1 null null"), core.calls)
    }

    @Test
    fun aFailedMoveShowsTheCoresMessage() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        core.failNext = MobileException.UnknownTask("This task is gone.")

        vm.moveTask("gone", "2026-11-24", null)

        assertEquals("This task is gone.", vm.message.value)
    }

    @Test
    fun cardActionsGoToTheCore() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        vm.toggleTask("p1")
        vm.startTimer("p2")
        vm.stopTimer()
        vm.deleteTask("p4")
        vm.undo()

        assertEquals(listOf("p1"), core.toggled)
        assertEquals(listOf("start p2", "stop", "delete p4", "undo"), core.calls)
    }
}
