package app.quest.android.ui.review

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

class ReviewViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        ReviewViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved)

    @Test
    fun readsThisWeekOnlyWhileShown() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(emptyList<String?>(), core.reviewWeeks)

        vm.setShown(true)

        assertEquals(ReviewUiState.Ready(QuestSamples.review), vm.state.value)
        assertEquals(listOf<String?>(null), core.reviewWeeks.distinct())
    }

    @Test
    fun arrowsAndBarsMoveByTheCoresWeeksAndThisWeekGoesBack() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        core.review = QuestSamples.lastWeek
        vm.previous()
        assertEquals("2026-11-16", core.reviewWeeks.last())
        vm.next()
        assertEquals("2026-11-23", core.reviewWeeks.last())
        vm.selectWeek("2026-10-12")
        assertEquals("2026-10-12", core.reviewWeeks.last())
        vm.thisWeek()
        assertEquals(null, core.reviewWeeks.last())
    }

    @Test
    fun theWeekSurvivesProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.setShown(true)
        vm.previous()

        viewModel(saved).setShown(true)

        assertEquals("2026-11-16", core.reviewWeeks.last())
    }
}
