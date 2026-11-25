package app.quest.android.ui.calendar

import androidx.compose.ui.semantics.heading
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.SheetValue
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import app.quest.android.R
import app.quest.android.ui.components.QuestChip
import app.quest.android.ui.components.QuestPickerChip
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.task.Caps
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.FormError
import uniffi.quest_mobile.FormField
import uniffi.quest_mobile.QuestChip as QuestRef

/** What the Log work sheet and the entry editor do, wired to [CalendarViewModel]. */
class CalendarSheetActions(
    val close: () -> Unit,
    val editLogWork: ((LogWorkForm) -> LogWorkForm) -> Unit,
    val saveLogWork: () -> Unit,
    val editEntry: ((EntryForm) -> EntryForm) -> Unit,
    val saveEntry: () -> Unit,
    val deleteEntry: () -> Unit,
    /** "Open task": closes the editor and opens the task's sheet. */
    val openTask: (taskId: String) -> Unit,
    val openTaskPicker: () -> Unit,
    val closeTaskPicker: () -> Unit,
    val editTaskQuery: (String) -> Unit,
    val pickTask: (String?) -> Unit,
    /** Log work: the search field's text becomes a new task's title. */
    val createTask: (title: String) -> Unit,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = CalendarSheetActions({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})
    }
}

/** The open sheet over the calendar (Log work or the entry editor); back closes it (predictive). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CalendarSheets(state: CalendarUiState, actions: CalendarSheetActions) {
    if (state !is CalendarUiState.Ready || (state.logWork == null && state.entry == null && state.sheetError == null)) return
    ModalBottomSheet(
        onDismissRequest = actions.close,
        // Full height only: the sheet is a form, a half-open state would hide its fields.
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = QuestTheme.colors.card,
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag("calendar-sheet"),
    ) {
        CalendarSheetContent(state, actions)
    }
}

/** The sheet's inside, without the sheet (screenshots draw it on its own). */
@Composable
fun CalendarSheetContent(state: CalendarUiState.Ready, actions: CalendarSheetActions) {
    val picker = state.picker
    BackHandler(enabled = picker != null, onBack = actions.closeTaskPicker)
    Column(
        Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = QuestSpacing.screen)
            .padding(bottom = QuestSpacing.screen)
            .navigationBarsPadding(),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
    ) {
        state.sheetError?.let { FieldError(it) }
        when {
            picker != null -> TaskPickerPage(picker, actions)
            state.logWork != null -> LogWorkForm(state.logWork, actions)
            state.entry != null -> EntryEditorForm(state.entry, actions)
        }
    }
}

/** The error to show at [field]; a missing task shows only as the disabled button. */
private fun FormError?.at(field: FormField): String? = this?.takeIf { it.field == field }?.message

@Composable
private fun SheetTitle(title: String) = Text(title, style = QuestType.stat, color = QuestTheme.colors.text, modifier = Modifier.semantics { heading() })

/** Log work as the desktop dialog: TASK, DATE, START, END, duration, QUEST; nothing saves before Log work. */
@Composable
private fun LogWorkForm(state: LogWorkState, actions: CalendarSheetActions) {
    val check = state.check
    val error = check.error
    val form = state.form
    SheetTitle(stringResource(R.string.log_work))
    PickerField(
        label = stringResource(R.string.form_task),
        value = check.taskTitle ?: stringResource(R.string.form_choose_task),
        placeholder = check.taskTitle == null,
        onClick = actions.openTaskPicker,
        tag = "log-work-task",
    )
    if (check.isNewTask) Text(stringResource(R.string.log_work_new_task), style = QuestType.small, color = QuestTheme.colors.textFaint)
    DateField(
        stringResource(R.string.form_date),
        check.date?.label ?: form.day,
        form.day,
        onPick = { day -> actions.editLogWork { it.copy(day = day) } },
        tag = "log-work-date",
        error = error.at(FormField.DATE),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
        TimeField(
            stringResource(R.string.form_start),
            check.startLabel,
            form.startMinute,
            onPick = { minute -> actions.editLogWork { it.copy(startMinute = minute) } },
            tag = "log-work-start",
            modifier = Modifier.weight(1f),
            error = error.at(FormField.START),
        )
        TimeField(
            stringResource(R.string.form_end),
            check.endLabel,
            form.endMinute,
            onPick = { minute -> actions.editLogWork { it.copy(endMinute = minute) } },
            tag = "log-work-end",
            modifier = Modifier.weight(1f),
            error = error.at(FormField.END),
        )
    }
    Duration(check.duration, "log-work-duration")
    // The session's quest follows its task: chosen only for a new task, else the task's own, read-only.
    if (check.canChooseQuest) {
        Caps(stringResource(R.string.task_quest))
        QuestPickerChip(check.quest, check.questOptions, { quest -> actions.editLogWork { it.copy(questId = quest?.id) } }, stringResource(R.string.task_quest), "form-quest-picker")
    } else {
        TaskQuest(check.quest)
    }
    state.saveError?.let { FieldError(it) }
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.weight(1f))
        TextButton(onClick = actions.close, modifier = Modifier.testTag("log-work-cancel")) { Text(stringResource(R.string.cancel)) }
        SaveButton(stringResource(R.string.log_work), enabled = error == null, onClick = actions.saveLogWork, tag = "log-work-save")
    }
}

/** The editor of a tracked session: task (its quest follows), start, end (maybe another day), duration. */
@Composable
private fun EntryEditorForm(state: EntryState, actions: CalendarSheetActions) {
    val check = state.check
    val error = check.error
    val form = state.form
    SheetTitle(stringResource(R.string.entry_editor_title))
    Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        PickerField(
            label = stringResource(R.string.form_task),
            value = check.taskTitle ?: stringResource(R.string.entry_no_task),
            placeholder = check.taskTitle == null,
            onClick = actions.openTaskPicker,
            tag = "entry-task",
            modifier = Modifier.weight(1f),
        )
        if (form.taskId != null) {
            IconButton(onClick = { actions.pickTask(null) }, modifier = Modifier.testTag("entry-clear-task")) {
                Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.entry_clear_task), tint = QuestTheme.colors.textMuted)
            }
        }
    }
    val taskId = form.taskId
    if (check.canOpenTask && taskId != null) {
        OutlinedButton(onClick = { actions.openTask(taskId) }, modifier = Modifier.testTag("entry-open-task")) {
            Text(stringResource(R.string.entry_open_task), style = QuestType.bodyMedium)
        }
    }
    TaskQuest(check.quest)
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
        DateField(
            stringResource(R.string.entry_start_date),
            check.startDate?.label ?: form.startDay,
            form.startDay,
            onPick = { day -> actions.editEntry { it.copy(startDay = day) } },
            tag = "entry-start-date",
            modifier = Modifier.weight(1.4f),
            error = error.at(FormField.DATE),
        )
        TimeField(
            stringResource(R.string.form_start),
            check.startLabel,
            form.startMinute,
            onPick = { minute -> actions.editEntry { it.copy(startMinute = minute) } },
            tag = "entry-start",
            modifier = Modifier.weight(1f),
            error = error.at(FormField.START),
        )
    }
    if (check.running) {
        PickerField(stringResource(R.string.form_end), stringResource(R.string.entry_running), onClick = null, placeholder = true, tag = "entry-end-running")
    } else {
        EndFields(state, actions)
    }
    Duration(check.duration, "entry-duration")
    state.saveError?.let { FieldError(it) }
    // Wraps Cancel and Save onto their own line when large text leaves no room.
    FlowRow(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small, Alignment.End), itemVerticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = actions.deleteEntry, modifier = Modifier.testTag("entry-delete")) {
            Icon(QuestIcons.Trash, contentDescription = null, tint = QuestTheme.colors.textMuted)
            Text(stringResource(R.string.task_delete), color = QuestTheme.colors.textMuted, modifier = Modifier.padding(start = QuestSpacing.small))
        }
        Box(Modifier.weight(1f))
        TextButton(onClick = actions.close, modifier = Modifier.testTag("entry-cancel")) { Text(stringResource(R.string.cancel)) }
        SaveButton(stringResource(R.string.save), enabled = error == null, onClick = actions.saveEntry, tag = "entry-save")
    }
}

/** END: the time, and with "Ends on a different day" its own date (over midnight). */
@Composable
private fun EndFields(state: EntryState, actions: CalendarSheetActions) {
    val check = state.check
    val form = state.form
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
        if (form.endsOtherDay) {
            DateField(
                stringResource(R.string.entry_end_date),
                check.endDate?.label ?: form.endDay,
                form.endDay,
                onPick = { day -> actions.editEntry { it.copy(endDay = day) } },
                tag = "entry-end-date",
                modifier = Modifier.weight(1.4f),
                error = check.error.at(FormField.END_DATE),
            )
        }
        TimeField(
            stringResource(R.string.form_end),
            check.endLabel,
            form.endMinute,
            onPick = { minute -> actions.editEntry { it.copy(endMinute = minute) } },
            tag = "entry-end",
            modifier = Modifier.weight(1f),
            error = check.error.at(FormField.END),
        )
    }
    Row(
        Modifier
            .heightIn(min = QuestSpacing.touchTarget)
            .toggleable(value = form.endsOtherDay, role = Role.Checkbox, onValueChange = { on -> actions.editEntry { it.copy(endsOtherDay = on) } })
            .testTag("entry-ends-other-day"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = form.endsOtherDay, onCheckedChange = null)
        Text(stringResource(R.string.entry_ends_other_day), style = QuestType.body, color = QuestTheme.colors.text, modifier = Modifier.padding(start = QuestSpacing.small))
    }
}

@Composable
private fun Duration(duration: String, tag: String) {
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Text(stringResource(R.string.form_duration), style = QuestType.small, color = QuestTheme.colors.textFaint)
        Text(duration, style = QuestType.smallMedium, color = QuestTheme.colors.text, modifier = Modifier.testTag(tag))
    }
}

@Composable
private fun SaveButton(label: String, enabled: Boolean, onClick: () -> Unit, tag: String) {
    Button(
        onClick = onClick,
        enabled = enabled,
        colors = ButtonDefaults.buttonColors(containerColor = QuestTheme.colors.text, contentColor = QuestTheme.colors.card),
        modifier = Modifier.testTag(tag),
    ) { Text(label, style = QuestType.bodyMedium) }
}

/** The task's quest, read-only; nothing without one (no task, or a task without a quest). */
@Composable
private fun TaskQuest(quest: QuestRef?) {
    if (quest == null) return
    Caps(stringResource(R.string.task_quest))
    QuestChip(quest, Modifier.testTag("form-quest"))
}
