package app.quest.android.ui.tracked

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
import app.quest.android.ui.logwork.DateField
import app.quest.android.ui.logwork.FieldError
import app.quest.android.ui.logwork.PickerField
import app.quest.android.ui.logwork.TaskPickerActions
import app.quest.android.ui.logwork.TaskPickerPage
import app.quest.android.ui.logwork.TimeField
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.FormError
import uniffi.quest_mobile.FormField
import uniffi.quest_mobile.SessionKind

class EntryActions(
    val close: () -> Unit,
    val edit: ((EntryForm) -> EntryForm) -> Unit,
    val save: () -> Unit,
    val delete: () -> Unit,
    val bookAsOrganizing: () -> Unit,
    val openTaskPicker: () -> Unit,
    val picker: TaskPickerActions,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = EntryActions({}, {}, {}, {}, {}, {}, TaskPickerActions({}, {}, {}, null))
    }
}

/** A tracked session over the Tracked tab; back closes it (or its task search). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EntrySheet(state: EntrySheetState?, actions: EntryActions) {
    if (state == null) return
    ModalBottomSheet(
        onDismissRequest = actions.close,
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = QuestTheme.colors.card,
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag("entry-sheet"),
    ) {
        EntrySheetContent(state, actions)
    }
}

/** The sheet's inside, without the sheet (screenshots draw it on its own). */
@Composable
fun EntrySheetContent(state: EntrySheetState, actions: EntryActions) {
    BackHandler(enabled = state is EntrySheetState.PickingTask, onBack = actions.picker.back)
    Column(
        Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = QuestSpacing.screen)
            .padding(bottom = QuestSpacing.screen)
            .navigationBarsPadding(),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
    ) {
        when (state) {
            is EntrySheetState.Failed -> FieldError(state.message)
            is EntrySheetState.PickingTask -> TaskPickerPage(state.picker, actions.picker)
            is EntrySheetState.Fields -> EntryFields(state, actions)
        }
    }
}

private fun FormError?.at(field: FormField): String? = this?.takeIf { it.field == field }?.message

@Composable
private fun EntryFields(state: EntrySheetState.Fields, actions: EntryActions) {
    val check = state.check
    val form = state.form
    val error = check.error
    val organizing = check.kind == SessionKind.ORGANIZING
    Text(stringResource(R.string.entry_title), style = QuestType.stat, color = QuestTheme.colors.text, modifier = Modifier.semantics { heading() })
    PickerField(
        label = stringResource(R.string.form_task),
        value = check.taskTitle ?: stringResource(if (organizing) R.string.organizing else R.string.entry_no_task),
        onClick = actions.openTaskPicker,
        tag = "entry-task",
    )
    check.quest?.let { QuestChip(it, Modifier.testTag("entry-quest")) }
    DateField(
        stringResource(R.string.form_date),
        check.startDate?.label ?: form.startDay,
        form.startDay,
        onPick = { day -> actions.edit { it.copy(startDay = day) } },
        tag = "entry-date",
        error = error.at(FormField.DATE),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
        TimeField(
            stringResource(R.string.form_start),
            check.startLabel,
            form.startMinute,
            onPick = { minute -> actions.edit { it.copy(startMinute = minute) } },
            tag = "entry-start",
            modifier = Modifier.weight(1f),
            error = error.at(FormField.START),
        )
        if (check.running) {
            PickerField(stringResource(R.string.form_end), stringResource(R.string.entry_running), onClick = null, tag = "entry-end", modifier = Modifier.weight(1f))
        } else {
            TimeField(
                stringResource(R.string.form_end),
                check.endLabel,
                form.endMinute,
                onPick = { minute -> actions.edit { it.copy(endMinute = minute) } },
                tag = "entry-end",
                modifier = Modifier.weight(1f),
                error = error.at(FormField.END),
            )
        }
    }
    if (form.endsOtherDay && !check.running) {
        DateField(
            stringResource(R.string.entry_end_date),
            check.endDate?.label ?: form.endDay,
            form.endDay,
            onPick = { day -> actions.edit { it.copy(endDay = day) } },
            tag = "entry-end-date",
            error = error.at(FormField.END_DATE),
        )
    }
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Text(stringResource(R.string.form_duration), style = QuestType.small, color = QuestTheme.colors.textFaint)
        Text(check.duration, style = QuestType.smallMedium, color = QuestTheme.colors.text, modifier = Modifier.testTag("entry-duration"))
    }
    state.saveError?.let { FieldError(it) }
    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = actions.delete, modifier = Modifier.testTag("entry-delete")) { Text(stringResource(R.string.delete), color = QuestTheme.colors.error) }
        if (!organizing) {
            TextButton(onClick = actions.bookAsOrganizing, modifier = Modifier.testTag("entry-organizing")) { Text(stringResource(R.string.organizing)) }
        }
        Box(Modifier.weight(1f))
        Button(
            onClick = actions.save,
            enabled = error == null,
            colors = ButtonDefaults.buttonColors(containerColor = QuestTheme.colors.text, contentColor = QuestTheme.colors.card),
            modifier = Modifier.testTag("entry-save"),
        ) { Text(stringResource(R.string.save), style = QuestType.bodyMedium) }
    }
}
