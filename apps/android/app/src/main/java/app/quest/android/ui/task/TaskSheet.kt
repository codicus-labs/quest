package app.quest.android.ui.task

import androidx.compose.ui.semantics.heading
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.SheetValue
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.CompleteToggle
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.QuestPickerChip
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.RepeatWeekday
import uniffi.quest_mobile.TaskDetail

/** Everything the task sheet can do, wired to [TaskSheetViewModel]. */
class TaskSheetActions(
    val close: () -> Unit,
    val editTitle: (String) -> Unit,
    val commitTitle: () -> Unit,
    val editDescription: (String) -> Unit,
    val commitDescription: () -> Unit,
    val toggleDone: () -> Unit,
    val setQuest: (String?) -> Unit,
    val setDay: (String?) -> Unit,
    val delete: () -> Unit,
    val openRepeat: () -> Unit,
    val editRepeat: ((RepeatForm) -> RepeatForm) -> Unit,
    val toggleWeekday: (RepeatWeekday) -> Unit,
    val saveRepeat: () -> Unit,
    val stopRepeating: () -> Unit,
    val closeRepeat: () -> Unit,
) {
    val repeatEditor get() = RepeatEditorActions(editRepeat, toggleWeekday, saveRepeat, stopRepeating, closeRepeat)

    companion object {
        /** Does nothing; for screenshots. */
        val None = TaskSheetActions({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})
    }
}

/** The task sheet (the desktop's task modal) over the current screen; back closes it (predictive). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TaskSheet(state: TaskSheetState, actions: TaskSheetActions) {
    if (state !is TaskSheetState.Open) return
    ModalBottomSheet(
        onDismissRequest = actions.close,
        // Full height only: the sheet is a form, a half-open state would hide its fields.
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = QuestTheme.colors.card,
        // The sheet is its own window, so it needs its own opt-in for UI automation ids.
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag("task-sheet"),
    ) {
        TaskSheetContent(state, actions)
    }
}

/** The sheet's inside, without the sheet (screenshots draw it on its own). */
@Composable
fun TaskSheetContent(state: TaskSheetState.Open, actions: TaskSheetActions) {
    val repeat = state.repeat
    BackHandler(enabled = repeat != null, onBack = actions.closeRepeat)
    Column(
        Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = QuestSpacing.screen)
            .padding(bottom = QuestSpacing.screen),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
    ) {
        if (repeat != null) {
            RepeatEditor(repeat, state.detail.title, state.detail.repeats, actions.repeatEditor)
        } else {
            TaskDetails(state, actions)
        }
    }
}

@Composable
private fun TaskDetails(state: TaskSheetState.Open, actions: TaskSheetActions) {
    val detail = state.detail
    Row(verticalAlignment = Alignment.Top) {
        CompleteToggle(detail.done, actions.toggleDone)
        CommitField(
            value = state.title,
            onEdit = { actions.editTitle(it.replace('\n', ' ')) },
            onCommit = actions.commitTitle,
            label = stringResource(R.string.task_title),
            singleLine = true,
            modifier = Modifier.weight(1f).padding(start = QuestSpacing.small).testTag("sheet-title"),
        )
    }
    HorizontalDivider(color = QuestTheme.colors.border)
    CommitField(
        value = state.description,
        onEdit = actions.editDescription,
        onCommit = actions.commitDescription,
        label = stringResource(R.string.task_description),
        placeholder = stringResource(R.string.task_description_placeholder),
        singleLine = false,
        modifier = Modifier.fillMaxWidth().heightIn(min = 96.dp).testTag("sheet-description"),
    )
    Caps(stringResource(R.string.task_date))
    DateRow(detail, actions.setDay)
    DueRow(detail)
    Caps(stringResource(R.string.task_repeat))
    RepeatRow(detail.repeatLabel, actions.openRepeat)
    Caps(stringResource(R.string.task_quest))
    QuestPickerChip(detail.quest, detail.questOptions, { actions.setQuest(it?.id) }, stringResource(R.string.task_assign_quest), "sheet-quest")
    LoggedTime(detail)
    TextButton(onClick = actions.delete, modifier = Modifier.testTag("sheet-delete")) {
        Icon(QuestIcons.Trash, contentDescription = null, tint = QuestTheme.colors.textMuted)
        Text(stringResource(R.string.task_delete_task), style = QuestType.body, color = QuestTheme.colors.textMuted, modifier = Modifier.padding(start = QuestSpacing.small))
    }
}

/** A text field that saves when the keyboard's action or leaving the field ends the edit. */
@Composable
fun CommitField(
    value: String,
    onEdit: (String) -> Unit,
    onCommit: () -> Unit,
    label: String,
    singleLine: Boolean,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
) {
    // Whether the field had focus yet: losing it only commits after it was gained.
    val hadFocus = remember { booleanArrayOf(false) }
    OutlinedTextField(
        value = value,
        onValueChange = onEdit,
        label = { Text(label) },
        placeholder = placeholder?.let { { Text(it) } },
        singleLine = singleLine,
        keyboardOptions = if (singleLine) KeyboardOptions(imeAction = ImeAction.Done) else KeyboardOptions.Default,
        keyboardActions = KeyboardActions(onDone = { onCommit() }),
        modifier = modifier.onFocusChanged { focus ->
            if (hadFocus[0] && !focus.isFocused) onCommit()
            hadFocus[0] = focus.isFocused
        },
    )
}

@Composable
fun Caps(text: String, modifier: Modifier = Modifier) {
    Text(text.uppercase(), style = QuestType.caps, color = QuestTheme.colors.textMuted, modifier = modifier.semantics { heading() })
}

@Composable
private fun DateRow(detail: TaskDetail, onSetDay: (String?) -> Unit) {
    var picking by rememberSaveable { mutableStateOf(false) }
    val colors = QuestTheme.colors
    Row(verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = { picking = true }, modifier = Modifier.testTag("sheet-date")) {
            Icon(Icons.Filled.DateRange, contentDescription = stringResource(R.string.task_choose_date), tint = colors.textMuted)
            Text(
                detail.plannedOn?.label ?: stringResource(R.string.task_backlog),
                style = QuestType.body,
                color = if (detail.plannedOn == null) colors.textFaint else colors.text,
                modifier = Modifier.padding(start = QuestSpacing.small),
            )
        }
        Box(Modifier.weight(1f))
        if (detail.plannedOn != null) {
            TextButton(onClick = { onSetDay(null) }, modifier = Modifier.testTag("sheet-backlog")) {
                Text(stringResource(R.string.task_move_to_backlog), style = QuestType.body, color = colors.textMuted)
            }
        }
    }
    if (picking) DayPickerDialog(detail.plannedOn?.iso, onPick = { onSetDay(it) }, onDismiss = { picking = false })
}

/** The deadline, read-only (set on the desktop or by capture); nothing without one. */
@Composable
private fun DueRow(detail: TaskDetail) {
    val due = detail.dueOn ?: return
    Caps(stringResource(R.string.task_deadline))
    Text(
        due.label,
        style = QuestType.body,
        color = if (detail.overdue) QuestTheme.colors.accent else QuestTheme.colors.text,
        modifier = Modifier.testTag("sheet-due"),
    )
}

@Composable
private fun RepeatRow(label: String, onOpen: () -> Unit) {
    val colors = QuestTheme.colors
    TextButton(onClick = onOpen, modifier = Modifier.testTag("sheet-repeat")) {
        Icon(QuestIcons.Repeat, contentDescription = null, tint = colors.textMuted)
        Text(label, style = QuestType.body, color = colors.text, modifier = Modifier.padding(start = QuestSpacing.small))
    }
}

@Composable
private fun LoggedTime(detail: TaskDetail) {
    val colors = QuestTheme.colors
    Row(verticalAlignment = Alignment.CenterVertically) {
        Caps(stringResource(R.string.task_logged_time))
        Box(Modifier.weight(1f))
        Text(detail.loggedTotal, style = QuestType.small, color = colors.textFaint, modifier = Modifier.testTag("sheet-logged-total"))
    }
    detail.logged.forEach { entry ->
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (entry.running) Dot(colors.accent, 7.dp, Modifier.padding(end = QuestSpacing.small))
            Text(entry.`when`, style = QuestType.small, color = colors.textMuted)
            Box(Modifier.weight(1f))
            Text(entry.duration, style = QuestType.small, color = colors.textFaint)
        }
    }
}
