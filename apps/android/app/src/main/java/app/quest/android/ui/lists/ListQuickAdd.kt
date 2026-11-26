package app.quest.android.ui.lists

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SheetValue
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import app.quest.android.ui.components.OnAccentFill
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.QuestBottomSheet
import app.quest.android.ui.components.questColor
import app.quest.android.ui.task.FieldPicker
import app.quest.android.ui.task.TaskCalendarChoices
import app.quest.android.ui.task.TaskFields
import app.quest.android.ui.task.TaskQuestChoices
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.CalendarMonth
import uniffi.quest_mobile.DateTarget
import uniffi.quest_mobile.QuestChip
import uniffi.quest_mobile.QuickFields

data class QuickAddFields(val fields: QuickFields, val quest: QuestChip?, val plan: String?, val deadline: String?)

class QuickAddActions(
    val questOptions: (String) -> List<QuestChip> = { emptyList() },
    val suggestions: (String) -> List<QuestChip> = { emptyList() },
    val setQuest: (String, String?) -> Unit = { _, _ -> },
    val setDate: (DateTarget, String?) -> Unit = { _, _ -> },
    val calendar: (String?, QuickFields, DateTarget) -> CalendarMonth? = { _, _, _ -> null },
    val shiftMonth: (String, Boolean) -> String? = { _, _ -> null },
    val editNote: (String) -> Unit = {},
)

@Composable
internal fun QuickAddFab(onClick: () -> Unit, modifier: Modifier = Modifier) {
    FloatingActionButton(
        onClick = onClick,
        modifier = modifier.testTag("list-add-open"),
        shape = CircleShape,
        containerColor = QuestTheme.colors.accentFill,
        contentColor = OnAccentFill,
    ) {
        Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.quick_add_title))
    }
}

@OptIn(ExperimentalMaterial3Api::class, androidx.compose.ui.ExperimentalComposeUiApi::class)
@Composable
internal fun ListQuickAddSheet(state: ListUiState.Ready, actions: ListActions, close: () -> Unit, error: String? = null) {
    ModalBottomSheet(
        onDismissRequest = close,
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = QuestTheme.colors.card,
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag("list-add-sheet"),
    ) {
        ListQuickAdd(state.draft, state.quick, actions, state.note, error)
    }
}

@Composable
internal fun ListQuickAdd(draft: String, fields: QuickAddFields?, actions: ListActions, note: String = "", error: String? = null) {
    val colors = QuestTheme.colors
    var focused by remember { mutableStateOf(false) }
    var input by rememberSaveable(stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue(draft, TextRange(draft.length))) }
    val value = if (input.text == draft) input else TextFieldValue(draft, TextRange(draft.length))
    val focus = remember { FocusRequester() }
    var picker by rememberSaveable { mutableStateOf<FieldPicker?>(null) }
    var showNote by rememberSaveable { mutableStateOf(note.isNotEmpty()) }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) {
        focus.requestFocus()
        keyboard?.show()
    }
    val quick = actions.quick
    fun submit() {
        focus.requestFocus()
        actions.add()
    }
    Column(
        Modifier.fillMaxWidth().imePadding().navigationBarsPadding()
            .verticalScroll(rememberScrollState()).padding(horizontal = QuestSpacing.screen)
            .padding(bottom = QuestSpacing.medium),
    ) {
        TextField(
            value = value,
            onValueChange = {
                input = it
                if (it.text != draft) actions.editDraft(it.text)
            },
            modifier = Modifier.fillMaxWidth().focusRequester(focus)
                .onFocusChanged { focused = it.isFocused }.testTag("list-add"),
            placeholder = { Text(stringResource(R.string.list_add), maxLines = 1) },
            leadingIcon = { Icon(Icons.Filled.Add, contentDescription = null, tint = colors.textMuted) },
            trailingIcon = if (draft.isNotBlank()) ({
                IconButton(onClick = ::submit, modifier = Modifier.testTag("list-add-send")) {
                    Icon(Icons.AutoMirrored.Filled.Send, contentDescription = stringResource(R.string.quick_add_save), tint = colors.text)
                }
            }) else null,
            singleLine = true,
            isError = error != null,
            supportingText = error?.let { { Text(it) } },
            shape = RoundedCornerShape(QuestSpacing.cardRadius),
            colors = TextFieldDefaults.colors(
                focusedContainerColor = colors.card,
                unfocusedContainerColor = colors.card,
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent,
                cursorColor = colors.accentLine,
            ),
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { submit() }),
        )
        if (fields != null) {
            TaskFields(fields.quest, fields.plan, fields.deadline, onOpen = {
                keyboard?.hide()
                picker = it
            })
        }
        if (focused && picker == null) {
            val caret = value.selection.end
            val before = draft.take(caret)
            val start = before.indexOfLast(Char::isWhitespace) + 1
            val token = before.substring(start)
            if (token.startsWith("#")) {
                val end = caret + draft.drop(caret).takeWhile { !it.isWhitespace() }.length
                quick.suggestions(token.drop(1)).take(4).forEach { quest ->
                    TextButton(onClick = { quick.setQuest(draft.removeRange(start, end), quest.id) }) {
                        Dot(questColor(quest.colorHex), 8.dp, modifier = Modifier.padding(end = QuestSpacing.small))
                        Text(quest.name, style = QuestType.small, color = colors.text)
                    }
                }
            }
        }
        TextButton(onClick = { showNote = !showNote }, modifier = Modifier.testTag("list-add-note-toggle")) {
            Text(stringResource(if (showNote) R.string.quick_add_hide_note else R.string.quick_add_note))
        }
        if (showNote) {
            OutlinedTextField(
                value = note,
                onValueChange = quick.editNote,
                label = { Text(stringResource(R.string.task_description)) },
                minLines = 2,
                maxLines = 4,
                modifier = Modifier.fillMaxWidth().testTag("list-add-note"),
            )
        }
    }
    val active = picker ?: return
    if (fields == null) return
    QuestBottomSheet(active.label, onDismiss = { picker = null }) {
        val target = active.target
        if (target == null) {
            TaskQuestChoices(quick.questOptions) {
                quick.setQuest(draft, it?.id)
                picker = null
            }
        } else {
            TaskCalendarChoices(target, { quick.calendar(it, fields.fields, target) }, quick.shiftMonth) {
                quick.setDate(target, it)
                picker = null
            }
        }
    }
}
