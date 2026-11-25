package app.quest.android.ui.quests

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.failureMessage
import java.io.Serializable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.QuestsData

/** The list the Quests tab shows. */
enum class QuestsTab { Active, Completed }

/** What the user picked on the Quests tab; survives process death. */
data class QuestsInputs(val tab: QuestsTab = QuestsTab.Active) : Serializable

sealed interface QuestsUiState {
    data object Loading : QuestsUiState

    data class Ready(val quests: QuestsData, val inputs: QuestsInputs) : QuestsUiState

    data class Failed(val message: String) : QuestsUiState
}

private const val INPUTS_KEY = "quests.inputs"

/** The Quests tab: the core builds the cards only while it is shown; this keeps the tab. The editor is [QuestEditorViewModel]. */
class QuestsViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private val inputs: StateFlow<QuestsInputs> = savedState.getStateFlow(INPUTS_KEY, QuestsInputs())

    val state: StateFlow<QuestsUiState> = combine(core.state, inputs, ::questsState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, QuestsUiState.Loading)

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    private fun update(change: (QuestsInputs) -> QuestsInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    fun setShown(shown: Boolean) = core.setQuestsShown(shown)

    fun selectTab(tab: QuestsTab) = update { it.copy(tab = tab) }

    fun undo() = report(core.perform { c, now -> c.undo(now) })

    fun dismissToast() = report(core.perform { c, now -> c.dismissToast(now) })

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }
}

private fun questsState(state: CoreState, inputs: QuestsInputs): QuestsUiState = when (state) {
    CoreState.Opening -> QuestsUiState.Loading
    is CoreState.Failed -> QuestsUiState.Failed(state.message)
    is CoreState.Ready -> state.quests?.let { QuestsUiState.Ready(it, inputs) } ?: QuestsUiState.Loading
}
