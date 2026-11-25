package app.quest.android.ui.task

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.Clock
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.RepeatRules
import app.quest.android.core.failureMessage
import java.io.Serializable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.RepeatCheck
import uniffi.quest_mobile.RepeatWeekday
import uniffi.quest_mobile.TaskDetail

/** The task sheet's two pages: the task itself, and its repeat settings. */
enum class SheetPage { Details, Repeat }

/** What the user is editing in the sheet; in the [SavedStateHandle], so it survives process death. */
data class SheetInputs(
    val taskId: String? = null,
    /** The title as typed, until saved; null shows the task's own. */
    val titleDraft: String? = null,
    val descriptionDraft: String? = null,
    val page: SheetPage = SheetPage.Details,
    val repeat: RepeatForm? = null,
    /** Why saving the repeat failed (the core's message), until the form changes. */
    val repeatError: String? = null,
) : Serializable

/** The repeat page: the form, the core's live check of it, and a save error. */
data class RepeatEditorState(val form: RepeatForm, val check: RepeatCheck, val saveError: String?) {
    /** The message to show under the form: a save failure, else the live check's. */
    val error: String? get() = saveError ?: check.error
}

sealed interface TaskSheetState {
    data object Closed : TaskSheetState

    data class Open(
        val detail: TaskDetail,
        val title: String,
        val description: String,
        val page: SheetPage,
        val repeat: RepeatEditorState?,
    ) : TaskSheetState
}

private const val INPUTS_KEY = "sheet.inputs"

private sealed interface DetailRead {
    data class Found(val detail: TaskDetail) : DetailRead

    data object Gone : DetailRead

    /** Reading failed ([message]), or the core is not open yet (null). */
    data class Failed(val message: String?) : DetailRead
}

/** The task sheet (the desktop's task modal): edits one task through the core. */
class TaskSheetViewModel(
    private val core: CoreHolder,
    private val savedState: SavedStateHandle,
    private val rules: RepeatRules,
    private val clock: Clock,
) : ViewModel() {
    private val inputs: StateFlow<SheetInputs> = savedState.getStateFlow(INPUTS_KEY, SheetInputs())

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    /** The task as last read, so a failed re-read keeps the sheet (and its drafts) open. */
    private var lastDetail: TaskDetail? = null

    val state: StateFlow<TaskSheetState> = combine(core.state, inputs) { _, inputs -> sheetState(inputs) }
        .stateIn(viewModelScope, SharingStarted.Eagerly, sheetState(inputs.value))

    init {
        // A task deleted elsewhere (or by sync) closes its sheet.
        viewModelScope.launch {
            state.collect { if (it == TaskSheetState.Closed && inputs.value.taskId != null && core.state.value is CoreState.Ready) clear() }
        }
    }

    private fun sheetState(inputs: SheetInputs): TaskSheetState {
        val taskId = inputs.taskId ?: return TaskSheetState.Closed
        val detail = when (val read = readDetail(taskId)) {
            DetailRead.Gone -> return TaskSheetState.Closed
            is DetailRead.Found -> read.detail.also { lastDetail = it }
            is DetailRead.Failed -> {
                read.message?.let { _message.value = it }
                lastDetail?.takeIf { it.id == taskId } ?: return TaskSheetState.Closed
            }
        }
        val repeat = inputs.repeat?.takeIf { inputs.page == SheetPage.Repeat }?.let { form ->
            RepeatEditorState(form, checkRepeat(form), inputs.repeatError)
        }
        return TaskSheetState.Open(
            detail = detail,
            title = inputs.titleDraft ?: detail.title,
            description = inputs.descriptionDraft ?: detail.description,
            page = if (repeat == null) SheetPage.Details else SheetPage.Repeat,
            repeat = repeat,
        )
    }

    /** The task; gone only when the core no longer knows it (deleted), not when a read fails. */
    private fun readDetail(taskId: String): DetailRead {
        val outcome = core.read { c, now ->
            try {
                c.taskDetail(taskId, now)
            } catch (_: MobileException.UnknownTask) {
                null
            }
        }
        return when (outcome) {
            is Outcome.Done -> outcome.value?.let { DetailRead.Found(it) } ?: DetailRead.Gone
            is Outcome.Failed -> DetailRead.Failed(outcome.message)
            Outcome.NotReady -> DetailRead.Failed(null)
        }
    }

    private fun checkRepeat(form: RepeatForm): RepeatCheck = try {
        rules.check(form.toDraft(), clock.now())
    } catch (e: MobileException) {
        RepeatCheck(shownWeekdays = form.weekdays, monthDayPlaceholder = "", summary = "", error = e.message, starts = null, ends = null)
    }

    private fun update(change: (SheetInputs) -> SheetInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    private fun openTask(): String? = inputs.value.taskId

    fun open(taskId: String) = update { SheetInputs(taskId = taskId) }

    /** Saves typed title and description, then closes. */
    fun close() {
        commitTitle()
        commitDescription()
        clear()
    }

    private fun clear() = update { SheetInputs() }

    fun editTitle(text: String) = update { it.copy(titleDraft = text) }

    /** Saves the typed title; the core ignores an empty or unchanged one (desktop rule). */
    fun commitTitle() {
        val taskId = openTask() ?: return
        val title = inputs.value.titleDraft ?: return
        report(core.perform { c, _ -> c.renameTask(taskId, title) })
        update { it.copy(titleDraft = null) }
    }

    fun editDescription(text: String) = update { it.copy(descriptionDraft = text) }

    fun commitDescription() {
        val taskId = openTask() ?: return
        val description = inputs.value.descriptionDraft ?: return
        report(core.perform { c, _ -> c.setTaskDescription(taskId, description) })
        update { it.copy(descriptionDraft = null) }
    }

    fun toggleDone() = act { taskId -> core.perform { c, now -> c.toggleTask(taskId, now) } }

    /** null: "No quest". */
    fun setQuest(questId: String?) = act { taskId -> core.perform { c, _ -> c.setTaskQuest(taskId, questId) } }

    /** [day] is ISO; null moves the task to the backlog. */
    fun setDay(day: String?) = act { taskId -> core.perform { c, now -> c.setTaskPlanned(taskId, day, now) } }

    /** Deletes the task and closes the sheet; Home shows the undo snackbar. */
    fun delete() {
        val taskId = openTask() ?: return
        val outcome = core.perform { c, now -> c.deleteTask(taskId, now) }
        report(outcome)
        if (outcome is Outcome.Done) clear()
    }

    /** Opens the repeat page with the desktop's starting values. */
    fun openRepeat() {
        val taskId = openTask() ?: return
        when (val outcome = core.perform { c, now -> c.repeatDraft(taskId, now) }) {
            is Outcome.Done -> update { it.copy(page = SheetPage.Repeat, repeat = RepeatForm.of(outcome.value), repeatError = null) }
            else -> report(outcome)
        }
    }

    fun editRepeat(change: (RepeatForm) -> RepeatForm) =
        update { inputs -> inputs.copy(repeat = inputs.repeat?.let(change), repeatError = null) }

    /** Taps a weekday chip (the core keeps at least one day selected, as the desktop does). */
    fun toggleWeekday(day: RepeatWeekday) {
        val form = inputs.value.repeat ?: return
        try {
            val next = rules.toggleWeekday(form.toDraft(), day, clock.now())
            editRepeat { RepeatForm.of(next) }
        } catch (e: MobileException) {
            update { it.copy(repeatError = e.message) }
        }
    }

    /** Saves the repeat; a rule the core rejects stays open with its message. */
    fun saveRepeat() {
        val taskId = openTask() ?: return
        val form = inputs.value.repeat ?: return
        when (val outcome = core.perform { c, now -> c.setTaskRepeat(taskId, form.toDraft(), now) }) {
            is Outcome.Done -> closeRepeat()
            else -> update { it.copy(repeatError = outcome.failureMessage()) }
        }
    }

    fun stopRepeating() {
        val taskId = openTask() ?: return
        val outcome = core.perform { c, now -> c.clearTaskRepeat(taskId, now) }
        report(outcome)
        if (outcome is Outcome.Done) closeRepeat()
    }

    fun closeRepeat() = update { it.copy(page = SheetPage.Details, repeat = null, repeatError = null) }

    fun dismissMessage() {
        _message.value = null
    }

    private fun act(action: (String) -> Outcome<*>) {
        val taskId = openTask() ?: return
        report(action(taskId))
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }
}
