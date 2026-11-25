package app.quest.android.ui.tracked

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.TrackedData

sealed interface TrackedUiState {
    data object Loading : TrackedUiState

    data class Ready(val tracked: TrackedData) : TrackedUiState

    data class Failed(val message: String) : TrackedUiState
}

/** The Tracked tab: today's timeline and this week; the core reads it only while it is shown. */
class TrackedViewModel(private val core: CoreHolder) : ViewModel() {
    val state: StateFlow<TrackedUiState> = core.state.map(::trackedState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, TrackedUiState.Loading)

    fun setShown(shown: Boolean) = core.setTrackedShown(shown)
}

private fun trackedState(state: CoreState): TrackedUiState = when (state) {
    CoreState.Opening -> TrackedUiState.Loading
    is CoreState.Failed -> TrackedUiState.Failed(state.message)
    is CoreState.Ready -> state.tracked?.let { TrackedUiState.Ready(it) } ?: TrackedUiState.Loading
}
