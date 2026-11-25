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

class QuestsViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        QuestsViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved)

    private val QuestsViewModel.ready get() = state.value as QuestsUiState.Ready

    @Test
    fun readsTheCardsOnlyWhileShown() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(QuestsUiState.Loading, vm.state.value)

        vm.setShown(true)
        assertEquals(QuestSamples.quests, vm.ready.quests)

        vm.setShown(false)
        assertEquals(QuestsUiState.Loading, vm.state.value)
    }

    @Test
    fun theTabSurvivesProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.setShown(true)
        vm.selectTab(QuestsTab.Completed)

        val restored = viewModel(saved)
        restored.setShown(true)

        assertEquals(QuestsInputs(QuestsTab.Completed), restored.ready.inputs)
    }

    @Test
    fun undoGoesToTheCore() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        vm.undo()
        vm.dismissToast()

        assertEquals(listOf("undo", "dismiss"), core.calls)
    }
}
