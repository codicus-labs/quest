package app.quest.android.ui.logwork

import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.heading
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import app.quest.android.R
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.TaskOption

/** What the task search does; without [create] it only picks existing tasks. */
class TaskPickerActions(
    val back: () -> Unit,
    val editQuery: (String) -> Unit,
    val pick: (String) -> Unit,
    val create: ((title: String) -> Unit)?,
)

/**
 * The task search of Log work and the entry sheet (desktop combobox): what was typed filters the
 * core's choices; with nothing matching, it offers "Create “…”" (a new, completed task) where
 * creating is allowed.
 */
@Composable
fun TaskPickerPage(state: TaskPickerState, actions: TaskPickerActions) {
    val colors = QuestTheme.colors
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val createTask = actions.create
    val create = state.choices.createLabel?.takeIf { createTask != null }
    Row(verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = actions.back, modifier = Modifier.testTag("task-picker-back")) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back), tint = colors.textMuted)
        }
        Text(stringResource(R.string.form_task), style = QuestType.stat, color = colors.text, modifier = Modifier.semantics { heading() })
    }
    // The field owns what is typed and reports it; mirroring the ViewModel back would drop fast typing.
    val query = rememberTextFieldState(state.query)
    LaunchedEffect(query) { snapshotFlow { query.text.toString() }.collect(actions.editQuery) }
    OutlinedTextField(
        state = query,
        placeholder = { Text(stringResource(R.string.task_picker_search_or_create)) },
        lineLimits = TextFieldLineLimits.SingleLine,
        // A fixed IME action: changing it while typing restarts the input and drops keys.
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        onKeyboardAction = { if (create != null) createTask?.invoke(query.text.toString()) },
        modifier = Modifier.fillMaxWidth().focusRequester(focus).testTag("task-picker-query"),
    )
    Column {
        create?.let { label ->
            OptionRow(onClick = { createTask?.invoke(query.text.toString()) }, tag = "task-picker-create") {
                Icon(Icons.Filled.Add, contentDescription = null, tint = colors.accent)
                Column(Modifier.weight(1f).padding(start = QuestSpacing.medium)) {
                    Text(label, style = QuestType.task, color = colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(stringResource(R.string.log_work_new_task), style = QuestType.small, color = colors.textFaint)
                }
            }
        }
        state.choices.options.forEach { option -> TaskOptionRow(option) { actions.pick(option.id) } }
        if (create == null && state.choices.options.isEmpty()) {
            Text(stringResource(R.string.task_picker_empty), style = QuestType.body, color = colors.textMuted, modifier = Modifier.padding(vertical = QuestSpacing.medium))
        }
    }
}

@Composable
private fun TaskOptionRow(option: TaskOption, onClick: () -> Unit) {
    val colors = QuestTheme.colors
    OptionRow(onClick = onClick, tag = "task-option:${option.title}") {
        Dot(option.quest?.let { questColor(it.colorHex) } ?: colors.textFaint, QuestSpacing.small)
        Text(
            option.title,
            style = QuestType.task,
            color = if (option.done) colors.textMuted else colors.text,
            textDecoration = if (option.done) TextDecoration.LineThrough else null,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).padding(start = QuestSpacing.medium),
        )
        Text(option.status, style = QuestType.small, color = colors.textFaint, modifier = Modifier.padding(start = QuestSpacing.small))
    }
}

@Composable
private fun OptionRow(onClick: () -> Unit, tag: String, content: @Composable RowScope.() -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget).clickable(onClick = onClick).padding(vertical = QuestSpacing.tight).testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.Start,
    ) { content() }
}
