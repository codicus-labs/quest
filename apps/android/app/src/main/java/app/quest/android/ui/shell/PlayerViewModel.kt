package app.quest.android.ui.shell

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.failureMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.PlayerCard

/** The mini player above the tabs: what runs (or ran last), Stop, and going back to a task. */
class PlayerViewModel(private val core: CoreHolder) : ViewModel() {
    val state: StateFlow<PlayerCard> = core.state.map { (it as? CoreState.Ready)?.player ?: PlayerCard.Empty }
        .stateIn(viewModelScope, SharingStarted.Eagerly, PlayerCard.Empty)

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    fun stop() = report(core.perform { c, now -> c.stopTimer(now) })

    /** Starts [taskId]'s timer (Resume, "Back to"); true when it started. */
    fun start(taskId: String): Boolean {
        val outcome = core.perform { c, now -> c.startTimer(taskId, now) }
        report(outcome)
        return outcome is Outcome.Done
    }

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }
}
