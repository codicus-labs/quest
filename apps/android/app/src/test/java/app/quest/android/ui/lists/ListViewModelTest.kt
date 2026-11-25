package app.quest.android.ui.lists

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeCore
import app.quest.android.ListSamples
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.ListKind
import uniffi.quest_mobile.MobileException

class ListViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        ListViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved)

    @Test
    fun readsOnlyTheShownList() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(ListUiState.Loading, vm.state.value)

        vm.show(ListKind.Inbox)
        assertEquals(ListUiState.Ready(ListSamples.inbox, ""), vm.state.value)

        vm.show(null)
        assertEquals(ListUiState.Loading, vm.state.value)
        assertTrue(core.listReads.all { it == ListKind.Inbox })
    }

    @Test
    fun addingSendsTheDraftToTheShownListAndClearsIt() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.show(ListKind.Quest("q3"))
        vm.editDraft("Sketch\nthe pricing")

        vm.add()

        assertEquals(listOf("add-to-list ${ListKind.Quest("q3")} Sketch the pricing"), core.calls)
        assertEquals("", (vm.state.value as ListUiState.Ready).draft)
    }

    @Test
    fun aBlankDraftAddsNothing() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.show(ListKind.Today)
        vm.editDraft("  ")

        vm.add()

        assertEquals(emptyList<String>(), core.calls)
    }

    @Test
    fun anotherListStartsWithAnEmptyDraftButComingBackKeepsIt() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.show(ListKind.Today)
        vm.editDraft("Call the landlord")
        vm.show(null)
        vm.show(ListKind.Today)
        assertEquals("Call the landlord", (vm.state.value as ListUiState.Ready).draft)

        vm.show(ListKind.Inbox)

        assertEquals("", (vm.state.value as ListUiState.Ready).draft)
    }

    @Test
    fun aDeletedQuestsListIsGone() = runTest(main.dispatcher) {
        val vm = viewModel()

        vm.show(ListKind.Quest("gone"))

        assertEquals(ListUiState.Gone, vm.state.value)
    }

    @Test
    fun aFailedActionShowsTheCoresReason() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.show(ListKind.Today)
        core.failNext = MobileException.Rejected("Already done.")

        assertEquals(false, vm.startTimer("t1"))

        assertEquals("Already done.", vm.message.value)
    }
}
