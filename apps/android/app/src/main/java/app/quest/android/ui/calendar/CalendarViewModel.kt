package app.quest.android.ui.calendar

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.QuestCoreApi
import app.quest.android.core.failureMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import uniffi.quest_mobile.CalendarData
import uniffi.quest_mobile.CalendarQuery
import uniffi.quest_mobile.EntryCheck
import uniffi.quest_mobile.GridDrag
import uniffi.quest_mobile.GridPlacement
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.Moment

sealed interface CalendarUiState {
    data object Loading : CalendarUiState

    data class Ready(
        val calendar: CalendarData,
        val logWork: LogWorkState?,
        val entry: EntryState?,
        /** The open sheet's task search; null while it shows its fields. */
        val picker: TaskPickerState?,
        /** A check the open sheet needs failed; the sheet stays open and shows it. */
        val sheetError: String? = null,
        /** The edited entry no longer exists (deleted elsewhere): the editor closes. */
        val entryGone: Boolean = false,
        /** The edited entry was running and has stopped (elsewhere): the editor reloads its draft. */
        val entryStopped: Boolean = false,
    ) : CalendarUiState

    data class Failed(val message: String) : CalendarUiState
}

private const val INPUTS_KEY = "calendar.inputs"

/**
 * The calendar: which page the core builds, drags on its grid, and the Log work and entry editor
 * sheets. Days, labels, snapping, checks and saving are the core's; this keeps the form fields.
 */
class CalendarViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private val inputs: StateFlow<CalendarInputs> = savedState.getStateFlow(INPUTS_KEY, CalendarInputs())
    private var shown = false

    /** Days side by side; the screen sets it from its width. */
    private var dayCount = 1

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    val state: StateFlow<CalendarUiState> = combine(core.state, inputs) { core, inputs -> calendarState(core, inputs) }
        .stateIn(viewModelScope, SharingStarted.Eagerly, CalendarUiState.Loading)

    init {
        // An entry deleted elsewhere (or by sync) closes its editor.
        viewModelScope.launch {
            state.collect {
                if (it !is CalendarUiState.Ready) return@collect
                if (it.entryGone) closeSheet()
                if (it.entryStopped) reloadStoppedEntry()
            }
        }
    }

    private fun update(change: (CalendarInputs) -> CalendarInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    /** The calendar came on screen with [dayCount] days side by side, or left it ([shown] false). */
    fun setShown(shown: Boolean, dayCount: Int = this.dayCount) {
        this.shown = shown
        this.dayCount = dayCount.coerceAtLeast(1)
        requery()
    }

    private fun requery() {
        core.setCalendarQuery(if (shown) CalendarQuery(inputs.value.firstDay, dayCount.toUInt()) else null)
    }

    private fun goTo(firstDay: String?) {
        update { it.copy(firstDay = firstDay) }
        requery()
    }

    fun previous() = page()?.let { goTo(it.previousFirstDay) }

    fun next() = page()?.let { goTo(it.nextFirstDay) }

    fun today() = goTo(null)

    private fun page(): CalendarData? = (core.state.value as? CoreState.Ready)?.calendar

    // ---- Drags on the grid ----

    /** The ghost for [drag] with the finger over [day] at [minute]; null where it can't land. Pure, in memory. */
    fun preview(drag: GridDrag, day: String, minute: Float): GridPlacement? =
        (core.read { c, now -> c.gridDragPreview(drag, day, minute, now) } as? Outcome.Done)?.value

    /** Drops [drag]: a move or resize saves (with Undo); a new span opens Log work for it. */
    fun drop(drag: GridDrag, day: String, minute: Float) {
        when (val outcome = core.perform { c, now -> c.gridDrop(drag, day, minute, now) }) {
            is Outcome.Done -> outcome.value?.let { draft -> openSheet(CalendarSheet.LogWork(LogWorkForm.of(draft))) }
            else -> report(outcome)
        }
    }

    fun undo() = report(core.perform { c, now -> c.undo(now) })

    fun dismissToast() = report(core.perform { c, now -> c.dismissToast(now) })

    // ---- Sheets ----

    private fun openSheet(sheet: CalendarSheet) = update { it.copy(sheet = sheet, pickingTask = false, taskQuery = "", saveError = null) }

    fun closeSheet() = openSheet(CalendarSheet.None)

    /** Log work from the button: the quarter hour that just ended, today. */
    fun openLogWork() {
        when (val outcome = core.read { c, now -> c.logWorkNow(now) }) {
            is Outcome.Done -> openSheet(CalendarSheet.LogWork(LogWorkForm.of(outcome.value)))
            else -> report(outcome)
        }
    }

    fun editLogWork(change: (LogWorkForm) -> LogWorkForm) = update { inputs ->
        val sheet = inputs.sheet as? CalendarSheet.LogWork ?: return@update inputs
        inputs.copy(sheet = CalendarSheet.LogWork(change(sheet.form)), saveError = null)
    }

    fun saveLogWork() {
        val form = (inputs.value.sheet as? CalendarSheet.LogWork)?.form ?: return
        when (val outcome = core.perform { c, now -> c.logWork(form.toDraft(), now) }) {
            is Outcome.Done -> closeSheet()
            else -> update { it.copy(saveError = outcome.failureMessage()) }
        }
    }

    /** Opens the editor for a tracked session (tap on its block). */
    fun openEntry(entryId: String) {
        when (val outcome = core.read { c, now -> readEntry(c, entryId, now) }) {
            is Outcome.Done -> openSheet(CalendarSheet.Entry(outcome.value))
            else -> report(outcome)
        }
    }

    /** The stored entry as a form, marked running while its timer runs. */
    private fun readEntry(c: QuestCoreApi, entryId: String, now: Moment): EntryForm {
        val draft = c.entryDraft(entryId, now)
        return EntryForm.of(draft, running = c.checkEntry(draft, now).running)
    }

    /**
     * The edited session was running and stopped elsewhere: its end is known now, so the form takes
     * the stored entry instead of keeping end = start (which the core would refuse). Unsaved edits in
     * the form are dropped: the stored entry is the truth now, and merging edits into it could save a
     * range the user never saw.
     */
    private fun reloadStoppedEntry() {
        val form = (inputs.value.sheet as? CalendarSheet.Entry)?.form ?: return
        when (val outcome = core.read { c, now -> readEntry(c, form.entryId, now) }) {
            is Outcome.Done -> update { it.copy(sheet = CalendarSheet.Entry(outcome.value), saveError = null) }
            else -> {
                // Not running any more either way; without this every later read would retry and report again.
                update { it.copy(sheet = CalendarSheet.Entry(form.copy(running = false))) }
                report(outcome)
            }
        }
    }

    fun editEntry(change: (EntryForm) -> EntryForm) = update { inputs ->
        val sheet = inputs.sheet as? CalendarSheet.Entry ?: return@update inputs
        inputs.copy(sheet = CalendarSheet.Entry(change(sheet.form)), saveError = null)
    }

    fun saveEntry() {
        val form = (inputs.value.sheet as? CalendarSheet.Entry)?.form ?: return
        when (val outcome = core.perform { c, now -> c.saveEntry(form.toDraft(), now) }) {
            is Outcome.Done -> closeSheet()
            else -> update { it.copy(saveError = outcome.failureMessage()) }
        }
    }

    /** Deletes the edited entry and closes the editor; the calendar's snackbar offers Undo. */
    fun deleteEntry() {
        val form = (inputs.value.sheet as? CalendarSheet.Entry)?.form ?: return
        val outcome = core.perform { c, now -> c.deleteEntry(form.entryId, now) }
        report(outcome)
        if (outcome is Outcome.Done) closeSheet()
    }

    // ---- The task search page ----

    fun openTaskPicker() = update { it.copy(pickingTask = true, taskQuery = "") }

    fun closeTaskPicker() = update { it.copy(pickingTask = false, taskQuery = "") }

    fun editTaskQuery(query: String) = update { it.copy(taskQuery = query) }

    /** Picks [taskId] (null: no task, the editor's "Other work") for the open sheet. */
    fun pickTask(taskId: String?) {
        when (inputs.value.sheet) {
            is CalendarSheet.LogWork -> if (taskId != null) editLogWork { it.copy(taskId = taskId, newTitle = "") }
            is CalendarSheet.Entry -> editEntry { it.copy(taskId = taskId) }
            CalendarSheet.None -> return
        }
        closeTaskPicker()
    }

    /** Log work: [title] (the search field's text) becomes a new, completed task; the core checks it. */
    fun createTask(title: String) {
        editLogWork { it.copy(taskId = null, newTitle = title) }
        closeTaskPicker()
    }

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }

    private fun calendarState(state: CoreState, inputs: CalendarInputs): CalendarUiState = when (state) {
        CoreState.Opening -> CalendarUiState.Loading
        is CoreState.Failed -> CalendarUiState.Failed(state.message)
        is CoreState.Ready -> state.calendar?.let { readyState(it, inputs) } ?: CalendarUiState.Loading
    }

    /** Pure: reads and checks only; what the user sees for a failure is in the state, not the message. */
    private fun readyState(calendar: CalendarData, inputs: CalendarInputs): CalendarUiState.Ready {
        var logWork: LogWorkState? = null
        var entry: EntryState? = null
        var sheetError: String? = null
        var entryGone = false
        var entryStopped = false
        when (val sheet = inputs.sheet) {
            is CalendarSheet.LogWork -> when (val checked = core.read { c, now -> c.checkLogWork(sheet.form.toDraft(), now) }) {
                is Outcome.Done -> logWork = LogWorkState(sheet.form, checked.value, inputs.saveError)
                else -> sheetError = checked.failureMessage()
            }
            is CalendarSheet.Entry -> when (val checked = checkEntry(sheet.form)) {
                is Outcome.Done -> {
                    val check = checked.value
                    if (check == null) entryGone = true else entry = EntryState(sheet.form, check, inputs.saveError)
                    if (check != null && sheet.form.running && !check.running) entryStopped = true
                }
                else -> sheetError = checked.failureMessage()
            }
            CalendarSheet.None -> Unit
        }
        var picker: TaskPickerState? = null
        if (inputs.pickingTask && (logWork != null || entry != null)) {
            when (val choices = core.read { c, _ -> c.taskChoices(inputs.taskQuery) }) {
                is Outcome.Done -> picker = TaskPickerState(inputs.taskQuery, choices.value, canCreate = logWork != null)
                else -> sheetError = choices.failureMessage()
            }
        }
        return CalendarUiState.Ready(calendar, logWork, entry, picker, sheetError, entryGone, entryStopped)
    }

    /** The entry's check; null when the core no longer knows the entry (deleted), not when a read fails. */
    private fun checkEntry(form: EntryForm): Outcome<EntryCheck?> = core.read { c, now ->
        try {
            c.checkEntry(form.toDraft(), now)
        } catch (_: MobileException.UnknownEntry) {
            null
        }
    }
}
