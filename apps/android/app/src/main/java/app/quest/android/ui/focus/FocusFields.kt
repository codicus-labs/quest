package app.quest.android.ui.focus

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType

/** Calls [onCommit] when the field loses focus after having had it. */
@Composable
private fun Modifier.commitOnBlur(onCommit: () -> Unit): Modifier {
    val hadFocus = remember { booleanArrayOf(false) }
    return onFocusChanged { focus ->
        if (hadFocus[0] && !focus.isFocused) onCommit()
        hadFocus[0] = focus.isFocused
    }
}

/** The title, big and centred, edited in place; Done or leaving the field saves it. */
@Composable
fun FocusTitle(title: String, onEdit: (String) -> Unit, onCommit: () -> Unit, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val focusManager = LocalFocusManager.current
    val label = stringResource(R.string.task_title)
    TextField(
        value = title,
        onValueChange = onEdit,
        textStyle = QuestType.focusTitle.copy(textAlign = TextAlign.Center),
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { focusManager.clearFocus() }),
        colors = TextFieldDefaults.colors(
            focusedContainerColor = Color.Transparent,
            unfocusedContainerColor = Color.Transparent,
            unfocusedIndicatorColor = Color.Transparent,
            focusedIndicatorColor = colors.accent,
            focusedTextColor = colors.text,
            unfocusedTextColor = colors.text,
            cursorColor = colors.accent,
        ),
        modifier = modifier
            .fillMaxWidth()
            .commitOnBlur(onCommit)
            .semantics { contentDescription = label }
            .testTag("focus-title"),
    )
}

/** The description, edited in place; leaving the field saves it (as in the task sheet). */
@Composable
fun FocusDescription(description: String, onEdit: (String) -> Unit, onCommit: () -> Unit, modifier: Modifier = Modifier) {
    OutlinedTextField(
        value = description,
        onValueChange = onEdit,
        label = { Text(stringResource(R.string.task_description)) },
        placeholder = { Text(stringResource(R.string.task_description_placeholder)) },
        textStyle = QuestType.body,
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = 96.dp)
            .commitOnBlur(onCommit)
            .testTag("focus-description"),
    )
}
