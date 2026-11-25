package app.quest.android.ui.plan

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.PlanPage
import app.quest.android.core.failureMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.PlanTaskList

sealed interface PlanUiState {
    data object Loading : PlanUiState
    data class Ready(val page: PlanPage) : PlanUiState
    data class Failed(val message: String) : PlanUiState
}

class PlanViewModel(private val core: CoreHolder) : ViewModel() {
    val state = core.state.map { state ->
        when (state) {
            CoreState.Opening -> PlanUiState.Loading
            is CoreState.Failed -> PlanUiState.Failed(state.message)
            is CoreState.Ready -> state.plan?.let(PlanUiState::Ready) ?: PlanUiState.Loading
        }
    }.stateIn(viewModelScope, SharingStarted.Eagerly, PlanUiState.Loading)

    private val _message = MutableStateFlow<String?>(null)
    val message = _message.asStateFlow()

    fun setShown(shown: Boolean) = core.setPlanShown(shown)

    fun placeTask(taskId: String, list: PlanTaskList, beforeTaskId: String?) =
        report(core.perform { c, now -> c.placeTask(taskId, list, beforeTaskId, now) })

    fun toggleTask(taskId: String) = report(core.perform { c, now -> c.toggleListTask(taskId, now) })

    fun startTimer(taskId: String): Boolean {
        val outcome = core.perform { c, now -> c.startTimer(taskId, now) }
        report(outcome)
        return outcome is Outcome.Done
    }

    fun stopTimer() = report(core.perform { c, now -> c.stopTimer(now) })

    fun undo() = report(core.perform { c, now -> c.undo(now) })

    fun dismissToast() = report(core.perform { c, now -> c.dismissToast(now) })

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }
}
