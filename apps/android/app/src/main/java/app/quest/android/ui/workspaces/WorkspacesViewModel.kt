package app.quest.android.ui.workspaces

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.failureMessage
import java.io.Serializable
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.WorkspacesData

/**
 * "Manage workspaces" as typed; in the SavedStateHandle, so the sheet and its typing survive
 * process death. Null [WorkspacesInputs.manage]: the sheet is closed.
 */
data class ManageInputs(
    val newName: String = "",
    /** Names typed into the live workspaces' fields, by id; a missing one shows the stored name. */
    val names: Map<String, String> = emptyMap(),
    /** The workspace whose delete waits for "Delete workspace", with the question it asks. */
    val confirmDelete: DeleteQuestion? = null,
    /** Why the last create, rename, delete or restore did not happen. */
    val error: String? = null,
) : Serializable

data class DeleteQuestion(val workspaceId: String, val text: String) : Serializable

data class WorkspacesInputs(val manage: ManageInputs? = null) : Serializable

sealed interface WorkspacesUiState {
    data object Loading : WorkspacesUiState

    data class Ready(val data: WorkspacesData, val manage: ManageInputs?) : WorkspacesUiState
}

private const val INPUTS_KEY = "workspaces.inputs"

/**
 * The switcher in the top bar and the "Manage workspaces" sheet (the desktop's switcher menu and
 * dialog): switch, create (and show), rename, delete with a confirmation that says what the
 * workspace holds, and restore. Names are checked by the core, with the desktop's messages.
 */
class WorkspacesViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private val inputs: StateFlow<WorkspacesInputs> = savedState.getStateFlow(INPUTS_KEY, WorkspacesInputs())

    val state: StateFlow<WorkspacesUiState> = combine(core.state, inputs, ::workspacesState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, workspacesState(core.state.value, inputs.value))

    /**
     * The shown workspace changed (a switch here, or another device deleted the shown one): the
     * app then drops what belonged to the old one (see `leaveWorkspace`). Not for the first value.
     */
    val switched: Flow<String> = core.state.filterIsInstance<CoreState.Ready>()
        .map { it.workspaces.active.id }
        .distinctUntilChanged()
        .drop(1)

    private val _message = MutableStateFlow<String?>(null)

    /** A failed switch from the menu, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    private fun update(change: (ManageInputs) -> ManageInputs) {
        val manage = inputs.value.manage ?: return
        savedState[INPUTS_KEY] = WorkspacesInputs(change(manage))
    }

    /** Shows [workspaceId]; true when it is shown now. */
    fun switchTo(workspaceId: String): Boolean {
        val outcome = core.perform { c, _ -> c.switchWorkspace(workspaceId) }
        outcome.failureMessage()?.let { _message.value = it }
        return outcome is Outcome.Done
    }

    fun openManage() {
        savedState[INPUTS_KEY] = WorkspacesInputs(ManageInputs())
    }

    fun closeManage() {
        savedState[INPUTS_KEY] = WorkspacesInputs()
    }

    fun editNewName(text: String) = update { it.copy(newName = text) }

    fun editName(workspaceId: String, text: String) = update { it.copy(names = it.names + (workspaceId to text)) }

    /** Creates the workspace named in "New workspace" and shows it; the sheet closes. */
    fun create() {
        val name = inputs.value.manage?.newName ?: return
        val outcome = core.perform { c, _ -> c.createWorkspace(name) }
        if (outcome is Outcome.Done) closeManage() else refuse(outcome)
    }

    /** Saves the name typed into [workspaceId]'s field; the field shows the saved name again. */
    fun rename(workspaceId: String) {
        val typed = inputs.value.manage?.names?.get(workspaceId) ?: return
        val outcome = core.perform { c, _ -> c.renameWorkspace(workspaceId, typed) }
        if (outcome is Outcome.Done) update { it.copy(names = it.names - workspaceId, error = null) } else refuse(outcome)
    }

    /** Asks to confirm deleting [workspaceId] with what it holds, or says why it can't go. */
    fun askDelete(workspaceId: String) {
        when (val outcome = core.read { c, _ -> c.deleteWorkspaceQuestion(workspaceId) }) {
            is Outcome.Done -> update { it.copy(confirmDelete = DeleteQuestion(workspaceId, outcome.value), error = null) }
            else -> refuse(outcome)
        }
    }

    fun cancelDelete() = update { it.copy(confirmDelete = null) }

    /** Deletes the workspace waiting for confirmation; the sheet stays open (Restore is in it). */
    fun confirmDelete() {
        val question = inputs.value.manage?.confirmDelete ?: return
        val outcome = core.perform { c, now -> c.deleteWorkspace(question.workspaceId, now) }
        if (outcome is Outcome.Done) update { it.copy(confirmDelete = null, names = it.names - question.workspaceId) } else refuse(outcome)
    }

    fun restore(workspaceId: String) {
        val outcome = core.perform { c, _ -> c.restoreWorkspace(workspaceId) }
        if (outcome is Outcome.Done) update { it.copy(error = null) } else refuse(outcome)
    }

    fun dismissMessage() {
        _message.value = null
    }

    private fun refuse(outcome: Outcome<*>) {
        val message = outcome.failureMessage() ?: return
        update { it.copy(error = message, confirmDelete = null) }
    }
}

private fun workspacesState(state: CoreState, inputs: WorkspacesInputs): WorkspacesUiState = when (state) {
    is CoreState.Ready -> WorkspacesUiState.Ready(state.workspaces, inputs.manage)
    else -> WorkspacesUiState.Loading
}
