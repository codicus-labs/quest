package app.quest.android.ui.logwork

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import app.quest.android.R
import app.quest.android.ui.components.QuestChip
import app.quest.android.ui.components.QuestPickerChip
import app.quest.android.ui.task.Caps
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.FormError
import uniffi.quest_mobile.FormField

/** What the Log work sheet does, wired to [LogWorkViewModel]. */
class LogWorkActions(
    val close: () -> Unit,
    val edit: ((LogWorkForm) -> LogWorkForm) -> Unit,
    val save: () -> Unit,
    val openTaskPicker: () -> Unit,
    val closeTaskPicker: () -> Unit,
    val editTaskQuery: (String) -> Unit,
    val pickTask: (String) -> Unit,
    /** The search field's text becomes a new task's title. */
    val createTask: (title: String) -> Unit,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = LogWorkActions({}, {}, {}, {}, {}, {}, {}, {})
    }
}

/** Log work over whatever is on screen; back closes it (predictive). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LogWorkSheet(state: LogWorkSheetState?, actions: LogWorkActions) {
    if (state == null) return
    ModalBottomSheet(
        onDismissRequest = actions.close,
        // Full height only: the sheet is a form, a half-open state would hide its fields.
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = QuestTheme.colors.card,
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag("log-work-sheet"),
    ) {
        LogWorkSheetContent(state, actions)
    }
}

/** The sheet's inside, without the sheet (screenshots draw it on its own). */
@Composable
fun LogWorkSheetContent(state: LogWorkSheetState, actions: LogWorkActions) {
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
        state.error?.let { FieldError(it) }
        when {
            picker != null -> TaskPickerPage(picker, actions)
            state.logWork != null -> LogWorkFields(state.logWork, actions)
        }
    }
}

/** The error to show at [field]; a missing task shows only as the disabled button. */
private fun FormError?.at(field: FormField): String? = this?.takeIf { it.field == field }?.message

/** Log work as the desktop dialog: TASK, DATE, START, END, duration, QUEST; nothing saves before Log work. */
@Composable
private fun LogWorkFields(state: LogWorkState, actions: LogWorkActions) {
    val check = state.check
    val error = check.error
    val form = state.form
    Text(stringResource(R.string.log_work), style = QuestType.stat, color = QuestTheme.colors.text, modifier = Modifier.semantics { heading() })
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
        onPick = { day -> actions.edit { it.copy(day = day) } },
        tag = "log-work-date",
        error = error.at(FormField.DATE),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
        TimeField(
            stringResource(R.string.form_start),
            check.startLabel,
            form.startMinute,
            onPick = { minute -> actions.edit { it.copy(startMinute = minute) } },
            tag = "log-work-start",
            modifier = Modifier.weight(1f),
            error = error.at(FormField.START),
        )
        TimeField(
            stringResource(R.string.form_end),
            check.endLabel,
            form.endMinute,
            onPick = { minute -> actions.edit { it.copy(endMinute = minute) } },
            tag = "log-work-end",
            modifier = Modifier.weight(1f),
            error = error.at(FormField.END),
        )
    }
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Text(stringResource(R.string.form_duration), style = QuestType.small, color = QuestTheme.colors.textFaint)
        Text(check.duration, style = QuestType.smallMedium, color = QuestTheme.colors.text, modifier = Modifier.testTag("log-work-duration"))
    }
    // The session's quest follows its task: chosen only for a new task, else the task's own, read-only.
    if (check.canChooseQuest) {
        Caps(stringResource(R.string.task_quest))
        QuestPickerChip(check.quest, check.questOptions, { quest -> actions.edit { it.copy(questId = quest?.id) } }, stringResource(R.string.task_quest), "form-quest-picker")
    } else {
        check.quest?.let { quest ->
            Caps(stringResource(R.string.task_quest))
            QuestChip(quest, Modifier.testTag("form-quest"))
        }
    }
    state.saveError?.let { FieldError(it) }
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.weight(1f))
        TextButton(onClick = actions.close, modifier = Modifier.testTag("log-work-cancel")) { Text(stringResource(R.string.cancel)) }
        Button(
            onClick = actions.save,
            enabled = error == null,
            colors = ButtonDefaults.buttonColors(containerColor = QuestTheme.colors.text, contentColor = QuestTheme.colors.card),
            modifier = Modifier.testTag("log-work-save"),
        ) { Text(stringResource(R.string.log_work), style = QuestType.bodyMedium) }
    }
}
