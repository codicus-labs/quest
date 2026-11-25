package app.quest.android.ui.logwork

import androidx.lifecycle.SavedStateHandle
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
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn

private const val INPUTS_KEY = "logwork.inputs"

/**
 * The Log work sheet (shortcut and search): checks, labels and saving are the core's; this keeps the
 * form fields. [state] is null while the sheet is closed.
 */
class LogWorkViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private val inputs: StateFlow<LogWorkInputs> = savedState.getStateFlow(INPUTS_KEY, LogWorkInputs())

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    val state: StateFlow<LogWorkSheetState?> = combine(core.state, inputs) { core, inputs -> sheetState(core, inputs) }
        .stateIn(viewModelScope, SharingStarted.Eagerly, null)

    private fun update(change: (LogWorkInputs) -> LogWorkInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    /** Opens Log work on the quarter hour that just ended, today. */
    fun open() {
        when (val outcome = core.read { c, now -> c.logWorkNow(now) }) {
            is Outcome.Done -> savedState[INPUTS_KEY] = LogWorkInputs(form = LogWorkForm.of(outcome.value))
            else -> report(outcome)
        }
    }

    fun close() {
        savedState[INPUTS_KEY] = LogWorkInputs()
    }

    fun edit(change: (LogWorkForm) -> LogWorkForm) = update { inputs ->
        val form = inputs.form ?: return@update inputs
        inputs.copy(form = change(form), saveError = null)
    }

    fun save() {
        val form = inputs.value.form ?: return
        when (val outcome = core.perform { c, now -> c.logWork(form.toDraft(), now) }) {
            is Outcome.Done -> close()
            else -> update { it.copy(saveError = outcome.failureMessage()) }
        }
    }

    // ---- The task search page ----

    fun openTaskPicker() = update { it.copy(pickingTask = true, taskQuery = "") }

    fun closeTaskPicker() = update { it.copy(pickingTask = false, taskQuery = "") }

    fun editTaskQuery(query: String) = update { it.copy(taskQuery = query) }

    fun pickTask(taskId: String) {
        edit { it.copy(taskId = taskId, newTitle = "") }
        closeTaskPicker()
    }

    /** [title] (the search field's text) becomes a new, completed task; the core checks it. */
    fun createTask(title: String) {
        edit { it.copy(taskId = null, newTitle = title) }
        closeTaskPicker()
    }

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }

    /** Pure: reads and checks only; what the user sees for a failure is in the state, not the message. */
    private fun sheetState(state: CoreState, inputs: LogWorkInputs): LogWorkSheetState? {
        val form = inputs.form ?: return null
        if (state !is CoreState.Ready) return null
        val logWork = when (val checked = core.read { c, now -> c.checkLogWork(form.toDraft(), now) }) {
            is Outcome.Done -> LogWorkState(form, checked.value, inputs.saveError)
            else -> return LogWorkSheetState(null, null, checked.failureMessage())
        }
        if (!inputs.pickingTask) return LogWorkSheetState(logWork, null)
        return when (val choices = core.read { c, _ -> c.taskChoices(inputs.taskQuery) }) {
            is Outcome.Done -> LogWorkSheetState(logWork, TaskPickerState(inputs.taskQuery, choices.value))
            else -> LogWorkSheetState(logWork, null, choices.failureMessage())
        }
    }
}
