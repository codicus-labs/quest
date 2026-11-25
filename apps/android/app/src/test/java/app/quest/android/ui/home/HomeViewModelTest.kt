package app.quest.android.ui.home

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeHomePreferences
import app.quest.android.Samples
import app.quest.android.core.CoreHolder
import app.quest.android.settings.HomePreferences
import app.quest.android.settings.HomePrefs
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.HomeQuery
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.SyncState
import uniffi.quest_mobile.TaskSection

class HomeViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()
    private val preferences = FakeHomePreferences()

    private fun kotlinx.coroutines.test.TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        HomeViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved, preferences)

    private val HomeViewModel.ready get() = state.value as HomeUiState.Ready

    @Test
    fun showsTheCoresHomeOnceOpen() = runTest(main.dispatcher) {
        val vm = viewModel()

        assertEquals(HomeUiState.Ready(Samples.home, SyncState.SignedOut, HomePrefs(), HomeInputs()), vm.state.value)
    }

    @Test
    fun cardActionsGoToTheCore() = runTest(main.dispatcher) {
        val vm = viewModel()

        vm.toggleTask("t1")
        vm.startTimer("t4")
        vm.stopTimer()
        vm.deleteTask("t3")
        vm.undo()
        vm.dismissToast()

        assertEquals(listOf("t1"), core.toggled)
        assertEquals(listOf("start t4", "stop", "delete t3", "undo", "dismiss"), core.calls)
    }

    @Test
    fun addsTheSectionsDraftWithFilterPickAndPositionThenClearsIt() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setQuestFilter("q2")
        vm.pickQuest("q1")
        vm.setAddPosition(AddPosition.BOTTOM)

        vm.editDraft(TaskSection.QUEST_WORK, "Draft intro")
        vm.addTask(TaskSection.QUEST_WORK)
        vm.editDraft(TaskSection.OTHER, "   ")
        vm.addTask(TaskSection.OTHER)

        assertEquals(listOf("add Draft intro QUEST_WORK BOTTOM q2 q1"), core.calls)
        assertEquals("", vm.ready.inputs.questWorkDraft)
        assertEquals("   ", vm.ready.inputs.otherDraft)
    }

    @Test
    fun failedAddKeepsTheDraftAndShowsTheMessage() = runTest(main.dispatcher) {
        val vm = viewModel()
        core.failNext = MobileException.Storage("disk full. Free some space.")

        vm.editDraft(TaskSection.QUEST_WORK, "Draft intro")
        vm.addTask(TaskSection.QUEST_WORK)

        assertEquals("Draft intro", vm.ready.inputs.questWorkDraft)
        assertEquals("disk full. Free some space.", vm.message.value)
    }

    @Test
    fun filterAndPickAreTheQueryHomeIsReadWith() = runTest(main.dispatcher) {
        val vm = viewModel()

        vm.setQuestFilter("q1")
        assertEquals(HomeQuery(questFilter = "q1", questPick = null), core.queries.last())

        vm.setQuestFilter(null)
        vm.pickQuest("q2")
        assertEquals(HomeQuery(questFilter = null, questPick = "q2"), core.queries.last())
    }

    @Test
    fun inputsSurviveProcessDeathThroughSavedState() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val first = viewModel(saved)
        first.setQuestFilter("q1")
        first.editDraft(TaskSection.OTHER, "Buy milk")

        val restored = viewModel(SavedStateHandle(saved.keys().associateWith { saved.get<Any>(it) }))

        assertEquals(
            HomeInputs(questFilter = "q1", otherDraft = "Buy milk"),
            restored.ready.inputs,
        )
        assertEquals(HomeQuery(questFilter = "q1", questPick = null), core.queries.last())
    }

    @Test
    fun homeWaitsForTheSavedPreferencesSoOtherDoesNotFlash() = runTest(main.dispatcher) {
        val loading = object : HomePreferences {
            override val prefs = MutableSharedFlow<HomePrefs>()

            override suspend fun setShowOther(show: Boolean) = Unit

            override suspend fun setAddPosition(position: AddPosition) = Unit
        }
        val vm = HomeViewModel(openedHolder(core, backgroundScope, main.dispatcher), SavedStateHandle(), loading)

        assertEquals(HomeUiState.Loading, vm.state.value)
        vm.editDraft(TaskSection.QUEST_WORK, "Too early")
        vm.addTask(TaskSection.QUEST_WORK)
        assertEquals(emptyList<String>(), core.calls)

        loading.prefs.emit(HomePrefs(showOther = false, addPosition = AddPosition.BOTTOM))
        assertEquals(HomePrefs(showOther = false, addPosition = AddPosition.BOTTOM), vm.ready.prefs)
    }

    @Test
    fun showOtherAndAddPositionArePersistedPreferences() = runTest(main.dispatcher) {
        val vm = viewModel()

        vm.setShowOther(false)
        vm.setAddPosition(AddPosition.BOTTOM)

        assertEquals(HomePrefs(showOther = false, addPosition = AddPosition.BOTTOM), preferences.prefs.value)
        assertEquals(HomePrefs(showOther = false, addPosition = AddPosition.BOTTOM), vm.ready.prefs)
    }

    @Test
    fun failedActionShowsTheCoresMessageUntilDismissed() = runTest(main.dispatcher) {
        val vm = viewModel()
        core.failNext = MobileException.TaskIsDone("This task is done. Reopen it to start the timer.")

        vm.startTimer("t2")
        assertEquals("This task is done. Reopen it to start the timer.", vm.message.value)

        vm.dismissMessage()
        assertNull(vm.message.value)
    }

    @Test
    fun openFailureIsShownInsteadOfTheList() = runTest(main.dispatcher) {
        val holder = CoreHolder(backgroundScope, main.dispatcher, FakeClock()) {
            throw MobileException.Storage("cannot open the data folder. Free some space and restart.")
        }

        val vm = HomeViewModel(holder, SavedStateHandle(), preferences)

        assertEquals(HomeUiState.Failed("cannot open the data folder. Free some space and restart."), vm.state.value)
    }
}
