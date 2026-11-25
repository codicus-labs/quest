package app.quest.android.ui.task

import androidx.compose.ui.semantics.heading
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import app.quest.android.ui.components.UndoSnackbar
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.CompleteToggle
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.QuestBottomSheet
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.CalendarMonth
import uniffi.quest_mobile.DateTarget
import uniffi.quest_mobile.LoggedEntry
import uniffi.quest_mobile.QuestChip
import uniffi.quest_mobile.QuickFields
import uniffi.quest_mobile.RepeatWeekday

/** Everything the task sheet can do, wired to [TaskSheetViewModel]. */
class TaskSheetActions(
    val close: () -> Unit,
    val editTitle: (String) -> Unit,
    val commitTitle: () -> Unit,
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
    val setDeadline: (String?) -> Unit = {},
    val questOptions: ((String) -> List<QuestChip>)? = null,
    val loggedEntries: (() -> List<LoggedEntry>)? = null,
    val calendar: (String?, QuickFields, DateTarget) -> CalendarMonth? = { _, _, _ -> null },
    val shiftMonth: (String, Boolean) -> String? = { _, _ -> null },
    val openEntry: (String) -> Unit = {},
    val notes: NoteActions = NoteActions.None,
    val checklist: ChecklistActions = ChecklistActions(),
    val undo: () -> Unit = {},
    val dismissToast: () -> Unit = {},
) {
    val repeatEditor get() = RepeatEditorActions(editRepeat, toggleWeekday, saveRepeat, stopRepeating, closeRepeat)

    companion object {
        /** Does nothing; for screenshots. */
        val None = TaskSheetActions({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})
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
        LifecycleEventEffect(Lifecycle.Event.ON_STOP) { actions.notes.save() }
        Box {
            TaskSheetContent(state, actions)
            state.toast?.let {
                UndoSnackbar(it.text, actions.undo, Modifier.align(Alignment.BottomCenter), it.actionLabel, actions.dismissToast)
            }
        }
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
    val colors = QuestTheme.colors
    var picker by rememberSaveable(detail.id) { mutableStateOf<FieldPicker?>(null) }
    var showEntries by rememberSaveable(detail.id) { mutableStateOf(false) }
    Row(verticalAlignment = Alignment.CenterVertically) {
        CompleteToggle(detail.done, actions.toggleDone)
        CommitField(
            value = state.title,
            onEdit = { actions.editTitle(it.replace('\n', ' ')) },
            onCommit = actions.commitTitle,
            placeholder = stringResource(R.string.task_title),
            style = QuestType.focusTitle.copy(color = colors.text),
            singleLine = true,
            modifier = Modifier.weight(1f).padding(start = QuestSpacing.small).testTag("sheet-title"),
        )
    }
    HorizontalDivider(color = QuestTheme.colors.border)
    TaskNote(detail.id, state.description, actions.notes, "sheet-description")
    TaskChecklist(detail.id, detail.checklist, actions.checklist, editable = true)
    TaskFields(
        detail.quest, detail.plannedOn?.label?.substringBefore(','), detail.dueOn?.label?.substringBefore(','),
        onOpen = { picker = it }, planMoves = detail.postponed.plan, deadlineMoves = detail.postponed.due,
    )
    TextButton(onClick = { showEntries = !showEntries }) {
        Text(
            "◷ ${detail.loggedTotal.removeSuffix(" total")} ${if (showEntries) "⌄" else "›"}",
            style = QuestType.small, color = QuestTheme.colors.textMuted,
            modifier = Modifier.testTag("sheet-logged-total"),
        )
    }
    if (showEntries) {
        (actions.loggedEntries?.invoke() ?: detail.logged).forEach { entry ->
            TextButton(onClick = { actions.openEntry(entry.id) }, modifier = Modifier.fillMaxWidth()) {
                if (entry.running) Dot(QuestTheme.colors.accent, 7.dp, Modifier.padding(end = QuestSpacing.small))
                Text(entry.`when`, color = QuestTheme.colors.textMuted, modifier = Modifier.weight(1f))
                Text(entry.duration, color = QuestTheme.colors.text)
            }
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        RepeatRow(detail.repeatLabel, actions.openRepeat)
        TextButton(onClick = actions.delete, modifier = Modifier.testTag("sheet-delete")) {
            Icon(QuestIcons.Trash, contentDescription = stringResource(R.string.task_delete_task), tint = QuestTheme.colors.textMuted)
        }
    }
    picker?.let { active ->
        QuestBottomSheet(active.label, onDismiss = { picker = null }) {
            val target = active.target
            if (target == null) {
                TaskQuestChoices(actions.questOptions ?: { detail.questOptions }) {
                    actions.setQuest(it?.id)
                    picker = null
                }
            } else {
                val fields = QuickFields(detail.quest?.id, detail.plannedOn?.iso, detail.dueOn?.iso)
                TaskCalendarChoices(target, { actions.calendar(it, fields, target) }, actions.shiftMonth) {
                    if (target == DateTarget.PLAN) actions.setDay(it) else actions.setDeadline(it)
                    picker = null
                }
            }
        }
    }
}

@Composable
private fun CommitField(
    value: String,
    onEdit: (String) -> Unit,
    onCommit: () -> Unit,
    placeholder: String,
    style: TextStyle,
    singleLine: Boolean,
    modifier: Modifier = Modifier,
) {
    val colors = QuestTheme.colors
    var hadFocus by remember { mutableStateOf(false) }
    BasicTextField(
        value = value,
        onValueChange = onEdit,
        textStyle = style,
        cursorBrush = SolidColor(colors.accent),
        decorationBox = { input ->
            Box(
                Modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget).padding(vertical = QuestSpacing.tight),
                contentAlignment = if (singleLine) Alignment.CenterStart else Alignment.TopStart,
            ) {
                if (value.isEmpty()) Text(placeholder, style = style, color = colors.textMuted)
                input()
            }
        },
        singleLine = singleLine,
        keyboardOptions = if (singleLine) KeyboardOptions(imeAction = ImeAction.Done) else KeyboardOptions.Default,
        keyboardActions = KeyboardActions(onDone = { onCommit() }),
        modifier = modifier.onFocusChanged { focus ->
            if (hadFocus && !focus.isFocused) onCommit()
            hadFocus = focus.isFocused
        },
    )
}

@Composable
fun Caps(text: String, modifier: Modifier = Modifier) {
    Text(text.uppercase(), style = QuestType.caps, color = QuestTheme.colors.textMuted, modifier = modifier.semantics { heading() })
}

@Composable
private fun RepeatRow(label: String, onOpen: () -> Unit) {
    val colors = QuestTheme.colors
    TextButton(onClick = onOpen, modifier = Modifier.testTag("sheet-repeat")) {
        Icon(QuestIcons.Repeat, contentDescription = null, tint = colors.textMuted)
        Text(label, style = QuestType.body, color = colors.text, modifier = Modifier.padding(start = QuestSpacing.small))
    }
}
