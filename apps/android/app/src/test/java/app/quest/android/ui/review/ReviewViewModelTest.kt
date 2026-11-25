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
    fun readsOnlyWhileShown() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(emptyList<Int>(), core.reviewOffsets)

        vm.setShown(true)
        assertEquals(ReviewUiState.Ready(QuestSamples.review), vm.state.value)
        assertEquals(listOf(0), core.reviewOffsets.distinct())

        vm.setShown(false)
        vm.changeWeek(-1)
        assertEquals(listOf(0), core.reviewOffsets.distinct())

        vm.setShown(true)
        assertEquals(ReviewUiState.Ready(QuestSamples.lastWeek), vm.state.value)
    }

    @Test
    fun theOffsetSurvivesProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.setShown(true)
        vm.changeWeek(-1)

        viewModel(saved).setShown(true)

        assertEquals(-1, core.reviewOffsets.last())
    }
}
