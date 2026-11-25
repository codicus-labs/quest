package app.quest.android.ui.tracked

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeCore
import app.quest.android.ListSamples
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.MobileException

class EntryViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel() = EntryViewModel(openedHolder(core, backgroundScope, main.dispatcher), SavedStateHandle())

    @Test
    fun opensTheSessionsFieldsWithTheCoresCheck() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertNull(vm.state.value)

        vm.open("e3")

        assertEquals(EntrySheetState.Fields(EntryForm.of(ListSamples.entryDraft), ListSamples.entryCheck, null), vm.state.value)
    }

    @Test
    fun anUnknownSessionShowsAMessageAndStaysClosed() = runTest(main.dispatcher) {
        val vm = viewModel()

        vm.open("e9")

        assertNull(vm.state.value)
        assertTrue(vm.message.value != null)
    }

    @Test
    fun savingSendsTheEditedTimesAndPickedTaskThenCloses() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("e3")
        vm.edit { it.copy(startMinute = 10 * 60 + 45) }
        vm.openTaskPicker()
        assertTrue(vm.state.value is EntrySheetState.PickingTask)
        vm.pickTask("t2")

        vm.save()

        assertEquals(listOf("save-entry e3 t2 645-720"), core.calls)
        assertNull(vm.state.value)
    }

    @Test
    fun aRefusedSaveKeepsTheSheetWithTheReason() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("e3")
        core.failNext = MobileException.Rejected("The end must be after the start.")

        vm.save()

        assertEquals("The end must be after the start.", (vm.state.value as EntrySheetState.Fields).saveError)
    }

    @Test
    fun deleteAndOrganizingActOnTheSessionAndClose() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open("e3")
        vm.bookAsOrganizing()
        assertNull(vm.state.value)
        vm.open("e3")
        vm.delete()

        assertEquals(listOf("organizing-entry e3", "delete-entry e3"), core.calls)
        assertNull(vm.state.value)
    }
}
