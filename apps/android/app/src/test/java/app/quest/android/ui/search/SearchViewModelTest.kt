package app.quest.android.ui.search

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeCore
import app.quest.android.QuestSamples
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.AppView
import uniffi.quest_mobile.SearchMode
import uniffi.quest_mobile.SearchOutcome
import uniffi.quest_mobile.SearchToken

class SearchViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        SearchViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved).apply { setShown(true) }

    private val SearchViewModel.ready get() = state.value as SearchUiState.Ready

    @Test
    fun searchesNothingWhileClosed() = runTest(main.dispatcher) {
        val vm = SearchViewModel(openedHolder(core, backgroundScope, main.dispatcher), SavedStateHandle())

        assertEquals(SearchUiState.Loading, vm.state.value)
        assertEquals(emptyList<Any>(), core.searches)
    }

    @Test
    fun listsTheCoresResultsForTheQuery() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(QuestSamples.commands, vm.ready.results)
        assertEquals("Search or jump…", vm.ready.placeholder)

        vm.editQuery("quest")

        assertEquals(listOf("Go to Quests"), vm.ready.results.map { it.title })
        assertEquals(SearchMode.Commands to "quest", core.searches.last())
    }

    @Test
    fun runningAResultReturnsItsOutcomeAndClearsTheField() = runTest(main.dispatcher) {
        core.searchOutcome = { SearchOutcome.Show(AppView.QUESTS) }
        val vm = viewModel()
        vm.editQuery("quest")

        assertEquals(SearchOutcome.Show(AppView.QUESTS), vm.run(SearchToken.Go(AppView.QUESTS)))
        assertEquals("", vm.ready.query)
    }

    @Test
    fun theKeyboardRunsTheFirstResult() = runTest(main.dispatcher) {
        core.searchOutcome = { token -> SearchOutcome.ShowQuest((token as SearchToken.ShowQuest).questId) }
        val vm = viewModel()
        vm.editQuery("book")

        assertEquals(SearchOutcome.ShowQuest("q1"), vm.runFirst())
    }

    @Test
    fun aChoiceSwitchesTheListAndStaysOpen() = runTest(main.dispatcher) {
        core.searchOutcome = { SearchOutcome.Choose(SearchMode.MoveTo("t1"), "Move to…") }
        val vm = viewModel()
        vm.editQuery("move")

        assertNull(vm.run(SearchToken.MoveTask("t1", null)))
        assertEquals(SavedSearchMode.MoveTo("t1"), vm.ready.mode)
        assertEquals("", vm.ready.query)
        assertEquals("Choose…", vm.ready.placeholder)
        assertEquals(SearchMode.MoveTo("t1") to "", core.searches.last())
    }

    @Test
    fun queryAndModeSurviveProcessDeath() = runTest(main.dispatcher) {
        core.searchOutcome = { SearchOutcome.Choose(SearchMode.ScheduleAt("t1"), "Schedule at…") }
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.run(SearchToken.MoveTask("t1", null))
        vm.editQuery("fri")

        val restored = viewModel(saved)

        assertEquals(SavedSearchMode.ScheduleAt("t1"), restored.ready.mode)
        assertEquals("fri", restored.ready.query)
    }
}
