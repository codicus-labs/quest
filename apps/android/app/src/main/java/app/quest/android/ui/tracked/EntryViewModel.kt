package app.quest.android.ui.tracked

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.failureMessage
import app.quest.android.ui.logwork.TaskPickerState
import java.io.Serializable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.EntryCheck
import uniffi.quest_mobile.EntryDraft

/** The entry sheet's fields ([EntryDraft]), kept in the SavedStateHandle. */
data class EntryForm(
    val entryId: String,
    val taskId: String?,
    val startDay: String,
    val startMinute: Int,
    val endDay: String,
    val endMinute: Int,
    val endsOtherDay: Boolean,
) : Serializable {
    fun toDraft() = EntryDraft(entryId, taskId, startDay, startMinute.toUInt(), endDay, endMinute.toUInt(), endsOtherDay)

    companion object {
        fun of(d: EntryDraft) = EntryForm(d.entryId, d.taskId, d.startDay, d.startMinute.toInt(), d.endDay, d.endMinute.toInt(), d.endsOtherDay)
    }
}

/** What the user changed in the sheet; [form] is null while it is closed. */
data class EntryInputs(
    val form: EntryForm? = null,
    val pickingTask: Boolean = false,
    val taskQuery: String = "",
    val saveError: String? = null,
) : Serializable

/** The open sheet: the fields and the core's check, or its task search, or a failed read. */
sealed interface EntrySheetState {
    data class Fields(val form: EntryForm, val check: EntryCheck, val saveError: String?) : EntrySheetState

    data class PickingTask(val picker: TaskPickerState) : EntrySheetState

    data class Failed(val message: String) : EntrySheetState
}

private const val INPUTS_KEY = "entry.inputs"

/**
 * A tracked session's sheet (a block on the Tracked timeline): start, end, task, Organizing and
 * Delete. Checks and saving are the core's. [state] is null while the sheet is closed.
 */
class EntryViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private val inputs: StateFlow<EntryInputs> = savedState.getStateFlow(INPUTS_KEY, EntryInputs())

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    val state: StateFlow<EntrySheetState?> = combine(core.state, inputs, ::sheetState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, null)

    private fun update(change: (EntryInputs) -> EntryInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    fun open(entryId: String) {
        when (val outcome = core.read { c, now -> c.entryDraft(entryId, now) }) {
            is Outcome.Done -> savedState[INPUTS_KEY] = EntryInputs(form = EntryForm.of(outcome.value))
            else -> report(outcome)
        }
    }

    fun close() {
        savedState[INPUTS_KEY] = EntryInputs()
    }

    fun edit(change: (EntryForm) -> EntryForm) = update { inputs ->
        val form = inputs.form ?: return@update inputs
        inputs.copy(form = change(form), saveError = null)
    }

    fun save() {
        val form = inputs.value.form ?: return
        when (val outcome = core.perform { c, now -> c.saveEntry(form.toDraft(), now) }) {
            is Outcome.Done -> close()
            else -> update { it.copy(saveError = outcome.failureMessage()) }
        }
    }

    /** Deletes the session; the core's toast offers Undo. */
    fun delete() {
        val entryId = inputs.value.form?.entryId ?: return
        closeAfter(core.perform { c, now -> c.deleteEntry(entryId, now) })
    }

    /** The session keeps its times and becomes Organizing. */
    fun bookAsOrganizing() {
        val entryId = inputs.value.form?.entryId ?: return
        closeAfter(core.perform { c, _ -> c.bookEntryAsOrganizing(entryId) })
    }

    private fun closeAfter(outcome: Outcome<*>) {
        if (outcome is Outcome.Done) close() else update { it.copy(saveError = outcome.failureMessage()) }
    }

    fun openTaskPicker() = update { it.copy(pickingTask = true, taskQuery = "") }

    fun closeTaskPicker() = update { it.copy(pickingTask = false, taskQuery = "") }

    fun editTaskQuery(query: String) = update { it.copy(taskQuery = query) }

    fun pickTask(taskId: String) {
        edit { it.copy(taskId = taskId) }
        closeTaskPicker()
    }

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }

    /** Pure: reads and checks only. */
    private fun sheetState(state: CoreState, inputs: EntryInputs): EntrySheetState? {
        val form = inputs.form ?: return null
        if (state !is CoreState.Ready) return null
        if (inputs.pickingTask) {
            return when (val choices = core.read { c, _ -> c.taskChoices(inputs.taskQuery) }) {
                is Outcome.Done -> EntrySheetState.PickingTask(TaskPickerState(inputs.taskQuery, choices.value))
                else -> EntrySheetState.Failed(choices.failureMessage().orEmpty())
            }
        }
        return when (val checked = core.read { c, now -> c.checkEntry(form.toDraft(), now) }) {
            is Outcome.Done -> EntrySheetState.Fields(form, checked.value, inputs.saveError)
            else -> EntrySheetState.Failed(checked.failureMessage().orEmpty())
        }
    }
}
