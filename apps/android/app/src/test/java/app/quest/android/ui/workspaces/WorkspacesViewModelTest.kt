package app.quest.android.ui.workspaces

import androidx.lifecycle.SavedStateHandle
import app.quest.android.DEFAULT_WORKSPACE
import app.quest.android.FakeCore
import app.quest.android.FakeWorkspaces
import app.quest.android.WorkspaceSamples
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class WorkspacesViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val workspaces = FakeWorkspaces(WorkspaceSamples.two)
    private val core = FakeCore(workspaces = workspaces)

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        WorkspacesViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved)

    private val WorkspacesViewModel.ready get() = state.value as WorkspacesUiState.Ready
    private val WorkspacesViewModel.manage get() = ready.manage!!

    @Test
    fun switchingShowsTheOtherWorkspaceAndTellsTheApp() = runTest(main.dispatcher) {
        val vm = viewModel()
        val switched = mutableListOf<String>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { vm.switched.toList(switched) }

        assertTrue(vm.switchTo("w2"))

        assertEquals("Work", vm.ready.data.active.name)
        assertEquals(listOf("w2"), switched)
        assertEquals(listOf("switch w2"), workspaces.calls)
    }

    @Test
    fun aRefusedSwitchSaysWhyAndShowsTheSameWorkspace() = runTest(main.dispatcher) {
        val vm = viewModel()
        workspaces.failure = "Workspace no longer exists."

        assertFalse(vm.switchTo("w2"))

        assertEquals("Workspace no longer exists.", vm.message.value)
        assertEquals(DEFAULT_WORKSPACE, vm.ready.data.active.id)
        vm.dismissMessage()
        assertNull(vm.message.value)
    }

    @Test
    fun creatingShowsTheNewWorkspaceAndClosesTheSheet() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.openManage()
        vm.editNewName("  Side  ")
        vm.create()

        assertEquals(listOf("create   Side  "), workspaces.calls)
        assertEquals("Side", vm.ready.data.active.name)
        assertNull(vm.ready.manage)
    }

    @Test
    fun aRefusedNameStaysTypedWithTheCoresReason() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.openManage()
        vm.editNewName("Work")
        workspaces.failure = "A workspace named “Work” already exists."
        vm.create()

        assertEquals("A workspace named “Work” already exists.", vm.manage.error)
        assertEquals("Work", vm.manage.newName)
    }

    @Test
    fun renamingSavesTheTypedNameOnly() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.openManage()
        vm.rename("w2")
        assertEquals(emptyList<String>(), workspaces.calls)

        vm.editName("w2", "Office")
        vm.rename("w2")
        assertEquals(listOf("rename w2 Office"), workspaces.calls)
        assertEquals(emptyMap<String, String>(), vm.manage.names)
    }

    @Test
    fun deletingAsksWithTheContentFirstAndCancelKeepsIt() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.openManage()
        vm.askDelete("w2")
        assertEquals(DeleteQuestion("w2", workspaces.question), vm.manage.confirmDelete)

        vm.cancelDelete()
        assertNull(vm.manage.confirmDelete)
        vm.askDelete("w2")
        vm.confirmDelete()

        assertEquals(listOf("ask delete w2", "ask delete w2", "delete w2"), workspaces.calls)
        assertNull(vm.manage.confirmDelete)
    }

    @Test
    fun theLastWorkspaceIsNotAskedAboutButExplained() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.openManage()
        workspaces.failure = "The last workspace cannot be deleted; create another one first."
        vm.askDelete(DEFAULT_WORKSPACE)

        assertNull(vm.manage.confirmDelete)
        assertEquals("The last workspace cannot be deleted; create another one first.", vm.manage.error)
    }

    @Test
    fun restoreAsksTheCore() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.openManage()
        vm.restore("w3")
        assertEquals(listOf("restore w3"), workspaces.calls)
    }

    @Test
    fun theSheetAndItsTypingSurviveProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val first = viewModel(saved)
        first.openManage()
        first.editNewName("Side")
        first.editName("w2", "Office")
        first.askDelete("w2")

        val restored = viewModel(SavedStateHandle(saved.keys().associateWith { saved.get<Any>(it) }))
        assertEquals(first.manage, restored.manage)
    }
}
