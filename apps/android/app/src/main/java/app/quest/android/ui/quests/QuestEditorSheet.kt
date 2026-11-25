package app.quest.android.ui.quests

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestBottomSheet
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.task.Caps
import app.quest.android.ui.task.CommitField
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.HueOption
import uniffi.quest_mobile.QuestHue

class QuestEditorActions(
    val close: () -> Unit,
    val editTitle: (String) -> Unit,
    /** The title field's Done: Create for a new quest, else save the title. */
    val titleDone: () -> Unit,
    val commitTitle: () -> Unit,
    val editDescription: (String) -> Unit,
    val commitDescription: () -> Unit,
    val setHue: (QuestHue) -> Unit,
    val create: () -> Unit,
    val toggleStatus: () -> Unit,
    val openPage: () -> Unit,
    val delete: () -> Unit,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = QuestEditorActions({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})
    }
}

/**
 * The quest editor as a sheet over the Quests tab (the desktop's "Edit quest" / "New quest" dialog).
 * Closing it (back, swipe, scrim) saves and returns to the tab as it was.
 */
@Composable
fun QuestEditorSheet(state: QuestEditorState, actions: QuestEditorActions) {
    val title = when (state) {
        QuestEditorState.Closed -> return
        is QuestEditorState.New -> stringResource(R.string.quests_new)
        is QuestEditorState.Editing -> stringResource(R.string.quest_edit)
    }
    QuestBottomSheet(title, onDismiss = actions.close) { QuestEditorFields(state, actions) }
}

/** The sheet's inside, also drawn on its own for screenshots; it scrolls (large text, keyboard). */
@Composable
fun QuestEditorFields(state: QuestEditorState, actions: QuestEditorActions, autoFocus: Boolean = true) = Column(
    Modifier.fillMaxWidth().verticalScroll(rememberScrollState()),
    verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
) {
    when (state) {
        QuestEditorState.Closed -> Unit
        is QuestEditorState.New -> {
            TitleField(state.title, state.error, actions, autoFocus)
            DescriptionField(state.description, actions)
            Colors(state.hues, actions.setHue)
            Button(
                onClick = actions.create,
                enabled = state.title.isNotBlank(),
                colors = ButtonDefaults.buttonColors(containerColor = QuestTheme.colors.accent, contentColor = QuestTheme.colors.card),
                modifier = Modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget).testTag("quest-editor-create"),
            ) {
                Icon(Icons.Filled.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                Text(stringResource(R.string.quests_create), modifier = Modifier.padding(start = QuestSpacing.small))
            }
        }
        is QuestEditorState.Editing -> {
            TitleField(state.title, error = null, actions, autoFocus = false)
            DescriptionField(state.description, actions)
            Colors(state.detail.hues, actions.setHue)
            Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
                WideButton(state.detail.statusAction, if (state.detail.badge == null) Icons.Filled.Check else QuestIcons.Repeat, "quest-editor-status", actions.toggleStatus)
                WideButton(stringResource(R.string.quest_open_page), QuestIcons.Open, "quest-editor-page", actions.openPage)
                Text(state.detail.summary, style = QuestType.small, color = QuestTheme.colors.textFaint, modifier = Modifier.testTag("quest-editor-figures"))
            }
            val deleteLabel = stringResource(R.string.quest_delete)
            TextButton(
                onClick = actions.delete,
                modifier = Modifier
                    .heightIn(min = QuestSpacing.touchTarget)
                    .semantics { contentDescription = deleteLabel }
                    .testTag("quest-editor-delete"),
            ) {
                Icon(QuestIcons.Trash, contentDescription = null, tint = QuestTheme.colors.error, modifier = Modifier.size(18.dp))
                Text(stringResource(R.string.delete), color = QuestTheme.colors.error, modifier = Modifier.padding(start = QuestSpacing.small))
            }
        }
    }
}

@Composable
private fun TitleField(title: String, error: String?, actions: QuestEditorActions, autoFocus: Boolean) {
    val focus = remember { FocusRequester() }
    if (autoFocus) LaunchedEffect(Unit) { focus.requestFocus() }
    // Whether the field had focus yet: losing it only saves after it was gained.
    val hadFocus = remember { booleanArrayOf(false) }
    OutlinedTextField(
        value = title,
        onValueChange = actions.editTitle,
        label = { Text(stringResource(R.string.quest_title)) },
        placeholder = { Text(stringResource(R.string.quest_title_placeholder)) },
        singleLine = true,
        textStyle = QuestType.stat,
        isError = error != null,
        supportingText = error?.let { { Text(it) } },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { actions.titleDone() }),
        modifier = Modifier
            .fillMaxWidth()
            .focusRequester(focus)
            .onFocusChanged {
                if (it.isFocused) hadFocus[0] = true else if (hadFocus[0]) actions.commitTitle()
            }
            .then(if (error != null) Modifier.semantics { error(error) } else Modifier)
            .testTag("quest-editor-title"),
    )
}

/** Markdown, as in the task sheet. */
@Composable
private fun DescriptionField(description: String, actions: QuestEditorActions) = CommitField(
    value = description,
    onEdit = actions.editDescription,
    onCommit = actions.commitDescription,
    label = stringResource(R.string.quest_description),
    singleLine = false,
    placeholder = stringResource(R.string.quest_description_placeholder),
    modifier = Modifier.fillMaxWidth().heightIn(min = 120.dp).testTag("quest-editor-description"),
)

@Composable
private fun Colors(hues: List<HueOption>, onPick: (QuestHue) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Caps(stringResource(R.string.quest_color))
        HuePicker(hues, onPick, tag = "quest-editor-hues")
    }
}

@Composable
private fun WideButton(label: String, icon: ImageVector, tag: String, onClick: () -> Unit) {
    OutlinedButton(onClick = onClick, modifier = Modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget).testTag(tag)) {
        Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
        Text(label, style = QuestType.bodyMedium, modifier = Modifier.padding(start = QuestSpacing.small))
    }
}
