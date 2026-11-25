package app.quest.android.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.quest.android.ui.theme.QuestTheme

/**
 * A title renamed in place (desktop `rename_field`): the field takes the [style] and the [modifier] of the
 * text it replaces, so nothing moves or resizes; only a thin accent line under it marks the edit. It wraps like
 * the text; [placeholder] shows while it is empty. Done, Enter (a hardware keyboard), hiding the keyboard or
 * leaving the field saves.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun InlineTitleField(
    value: String,
    onEdit: (String) -> Unit,
    onCommit: () -> Unit,
    style: TextStyle,
    placeholder: String,
    modifier: Modifier = Modifier,
    maxLines: Int = Int.MAX_VALUE,
) {
    val colors = QuestTheme.colors
    val focus = remember { FocusRequester() }
    val focusManager = LocalFocusManager.current
    // Whether the field had focus yet: losing it only commits after it was gained.
    val hadFocus = remember { booleanArrayOf(false) }
    LaunchedEffect(Unit) { focus.requestFocus() }
    // Back hides the keyboard but keeps the focus; without a keyboard there is nothing left to type, so the
    // rename ends there (as a click elsewhere ends it on the desktop). Hardware keyboards never show one.
    val imeVisible = WindowInsets.isImeVisible
    val imeWasVisible = remember { booleanArrayOf(false) }
    LaunchedEffect(imeVisible) {
        if (imeWasVisible[0] && !imeVisible) focusManager.clearFocus()
        imeWasVisible[0] = imeVisible
    }
    BasicTextField(
        value = value,
        onValueChange = onEdit,
        textStyle = style.copy(color = colors.text),
        cursorBrush = SolidColor(colors.accent),
        maxLines = maxLines,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { onCommit() }),
        decorationBox = { field ->
            Box {
                if (value.isEmpty()) Text(placeholder, style = style, color = colors.textFaint)
                field()
            }
        },
        modifier = modifier
            .drawBehind {
                val y = size.height - 1.dp.toPx()
                drawLine(colors.accent, Offset(0f, y), Offset(size.width, y), strokeWidth = 1.dp.toPx())
            }
            .focusRequester(focus)
            // Wrapping like the text, the field is multi-line: Enter would insert a line break.
            .onPreviewKeyEvent { event ->
                val enter = event.key == Key.Enter || event.key == Key.NumPadEnter
                if (enter && event.type == KeyEventType.KeyDown) onCommit()
                enter
            }
            .onFocusChanged { state ->
                if (hadFocus[0] && !state.isFocused) onCommit()
                hadFocus[0] = state.isFocused
            },
    )
}
