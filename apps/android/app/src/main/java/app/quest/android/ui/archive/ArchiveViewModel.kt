package app.quest.android.ui.archive

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.failureMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.ArchiveData
import uniffi.quest_mobile.TrashKind

sealed interface ArchiveUiState {
    data object Loading : ArchiveUiState

    data class Ready(val archive: ArchiveData) : ArchiveUiState

    data class Failed(val message: String) : ArchiveUiState
}

/** The archive: archived quests and what was deleted, grouped by the core; read only while shown. */
class ArchiveViewModel(private val core: CoreHolder) : ViewModel() {
    val state: StateFlow<ArchiveUiState> = core.state.map(::archiveState).stateIn(viewModelScope, SharingStarted.Eagerly, ArchiveUiState.Loading)

    private val _message = MutableStateFlow<String?>(null)

    /** A failed restore or unarchive, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    fun setShown(shown: Boolean) = core.setArchiveShown(shown)

    fun restore(kind: TrashKind, id: String) {
        core.perform { c, now -> c.restore(kind, id, now) }.failureMessage()?.let { _message.value = it }
    }

    fun unarchive(questId: String) {
        core.perform { c, _ -> c.unarchiveQuest(questId) }.failureMessage()?.let { _message.value = it }
    }

    fun dismissMessage() {
        _message.value = null
    }
}

private fun archiveState(state: CoreState): ArchiveUiState = when (state) {
    CoreState.Opening -> ArchiveUiState.Loading
    is CoreState.Failed -> ArchiveUiState.Failed(state.message)
    is CoreState.Ready -> state.archive?.let { ArchiveUiState.Ready(it) } ?: ArchiveUiState.Loading
}
