package app.quest.android.quickadd

import androidx.compose.ui.semantics.heading
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.QuestPickerChip
import app.quest.android.ui.task.Caps
import app.quest.android.ui.task.DayPickerDialog
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType

class QuickAddActions(
    val close: () -> Unit,
    val editTitle: (String) -> Unit,
    val editDescription: (String) -> Unit,
    val setQuest: (String?) -> Unit,
    val setDay: (String?) -> Unit,
    val add: () -> Unit,
    /** The mic: dictate into AI capture instead; null hides it (AI capture off or not set up). */
    val dictate: (() -> Unit)? = null,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = QuickAddActions({}, {}, {}, {}, {}, {})
    }
}

/** The quick-add sheet over whatever was on screen (its activity is transparent); closing it ends the activity. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QuickAddSheet(state: QuickAddUiState, actions: QuickAddActions) {
    ModalBottomSheet(
        onDismissRequest = actions.close,
        // Full height only, app-wide: half open, the keyboard would cover the fields and buttons.
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = QuestTheme.colors.card,
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag("quick-add"),
    ) {
        QuickAddContent(state, actions)
    }
}

/** The sheet's inside, without the sheet (screenshots draw it on its own). */
@Composable
fun QuickAddContent(state: QuickAddUiState, actions: QuickAddActions) {
    Column(
        Modifier
            .fillMaxWidth()
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = QuestSpacing.screen)
            .padding(bottom = QuestSpacing.screen)
            .navigationBarsPadding(),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
    ) {
        Text(stringResource(R.string.quick_add_title), style = QuestType.stat, color = QuestTheme.colors.text, modifier = Modifier.semantics { heading() })
        when (state) {
            QuickAddUiState.Loading, QuickAddUiState.Added -> Box(Modifier.fillMaxWidth().heightIn(min = 160.dp), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            is QuickAddUiState.Failed -> Text(state.message ?: stringResource(R.string.quick_add_core_failed), style = QuestType.body, color = QuestTheme.colors.textMuted)
            is QuickAddUiState.Ready -> Form(state, actions)
        }
    }
}

@Composable
private fun Form(state: QuickAddUiState.Ready, actions: QuickAddActions) {
    val inputs = state.inputs
    val titleFocus = remember { FocusRequester() }
    // Only on a fresh sheet; after rotation or process death the keyboard state comes back by itself.
    var focused by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        if (!focused) titleFocus.requestFocus()
        focused = true
    }
    OutlinedTextField(
        value = inputs.title,
        onValueChange = actions.editTitle,
        label = { Text(stringResource(R.string.task_title)) },
        singleLine = true,
        isError = state.error != null,
        supportingText = state.error?.let { { Text(it, modifier = Modifier.semantics { error(it) }) } },
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Next),
        trailingIcon = actions.dictate?.let { dictate ->
            {
                IconButton(onClick = dictate, modifier = Modifier.testTag("quick-add-dictate")) {
                    Icon(QuestIcons.Mic, contentDescription = stringResource(R.string.capture_dictate))
                }
            }
        },
        modifier = Modifier.fillMaxWidth().focusRequester(titleFocus).testTag("quick-add-title"),
    )
    OutlinedTextField(
        value = inputs.description,
        onValueChange = actions.editDescription,
        label = { Text(stringResource(R.string.task_description)) },
        placeholder = { Text(stringResource(R.string.task_description_placeholder)) },
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
        modifier = Modifier.fillMaxWidth().heightIn(min = 96.dp).testTag("quick-add-description"),
    )
    Caps(stringResource(R.string.task_quest))
    val quest = state.form.questOptions.firstOrNull { it.id == inputs.questId }
    QuestPickerChip(quest, state.form.questOptions, { actions.setQuest(it?.id) }, stringResource(R.string.task_quest), "quick-add-quest")
    Caps(stringResource(R.string.task_date))
    DayChoice(state, actions.setDay)
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = actions.close, modifier = Modifier.testTag("quick-add-cancel")) { Text(stringResource(R.string.cancel)) }
        Button(onClick = actions.add, modifier = Modifier.padding(start = QuestSpacing.small).testTag("quick-add-save")) {
            Text(stringResource(R.string.quick_add_save))
        }
    }
}

/** The day (today preset; the picker for another) or the backlog. */
@Composable
private fun DayChoice(state: QuickAddUiState.Ready, onSetDay: (String?) -> Unit) {
    var picking by rememberSaveable { mutableStateOf(false) }
    val colors = QuestTheme.colors
    val day = state.day
    Row(verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = { picking = true }, modifier = Modifier.testTag("quick-add-date")) {
            Icon(Icons.Filled.DateRange, contentDescription = stringResource(R.string.task_choose_date), tint = colors.textMuted)
            Text(
                day?.label ?: stringResource(R.string.task_backlog),
                style = QuestType.body,
                color = if (day == null) colors.textFaint else colors.text,
                modifier = Modifier.padding(start = QuestSpacing.small),
            )
        }
        Box(Modifier.weight(1f))
        if (day != null) {
            TextButton(onClick = { onSetDay(null) }, modifier = Modifier.testTag("quick-add-backlog")) {
                Text(stringResource(R.string.task_move_to_backlog), style = QuestType.body, color = colors.textMuted)
            }
        } else {
            TextButton(onClick = { onSetDay(state.form.today.iso) }, modifier = Modifier.testTag("quick-add-today")) {
                Text(stringResource(R.string.quick_add_today), style = QuestType.body, color = colors.textMuted)
            }
        }
    }
    if (picking) DayPickerDialog(day?.iso ?: state.form.today.iso, onPick = onSetDay, onDismiss = { picking = false })
}
