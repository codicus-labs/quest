package app.quest.android.ui.quests

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.QuestNav

sealed interface QuestNavUiState {
    data object Loading : QuestNavUiState

    data class Ready(val nav: QuestNav) : QuestNavUiState

    data class Failed(val message: String) : QuestNavUiState
}

/** The Quests tab: the core reads it only while it is shown. */
class QuestNavViewModel(private val core: CoreHolder) : ViewModel() {
    val state: StateFlow<QuestNavUiState> = core.state.map(::questNavState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, QuestNavUiState.Loading)

    /** Also reads the quests' data, which the editor's "New quest" takes its free colours from. */
    fun setShown(shown: Boolean) {
        core.setQuestNavShown(shown)
        core.setQuestsShown(shown)
    }
}

private fun questNavState(state: CoreState): QuestNavUiState = when (state) {
    CoreState.Opening -> QuestNavUiState.Loading
    is CoreState.Failed -> QuestNavUiState.Failed(state.message)
    is CoreState.Ready -> state.questNav?.let { QuestNavUiState.Ready(it) } ?: QuestNavUiState.Loading
}
