package app.quest.android.ui.calendar

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.text.style.TextOverflow
import app.quest.android.R
import app.quest.android.ui.task.Caps
import app.quest.android.ui.task.DayPickerDialog
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType

/**
 * A form value shown as a tappable box (a date, a time, the task) under its caps [label]; tapping
 * opens its picker. [error] shows under it in the accent colour, as at the desktop field. Without
 * [onClick] it only shows the value (read-only).
 */
@Composable
fun PickerField(
    label: String,
    value: String,
    onClick: (() -> Unit)?,
    modifier: Modifier = Modifier,
    placeholder: Boolean = false,
    error: String? = null,
    tag: String,
) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.fieldRadius)
    Column(modifier, verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Caps(label)
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = QuestSpacing.touchTarget)
                .border(QuestSpacing.hairline, if (error != null) colors.accent else colors.borderStrong, shape)
                .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
                .clearAndSetSemantics {
                    contentDescription = "$label: $value"
                    error?.let { error(it) }
                    if (onClick != null) {
                        role = Role.Button
                        onClick { onClick(); true }
                    }
                }
                .padding(horizontal = QuestSpacing.medium)
                .testTag(tag),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                value,
                style = QuestType.task,
                color = if (placeholder) colors.textMuted else colors.text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            if (onClick != null) Icon(Icons.Filled.ArrowDropDown, contentDescription = null, tint = colors.textMuted)
        }
        error?.let { FieldError(it) }
    }
}

@Composable
fun FieldError(message: String, modifier: Modifier = Modifier) {
    Text(message, style = QuestType.small, color = QuestTheme.colors.accent, modifier = modifier.testTag("form-error"))
}

/** A date field: its label ("Nov 23, 2026") opens the Material date picker on [iso]. */
@Composable
fun DateField(label: String, value: String, iso: String, onPick: (String) -> Unit, tag: String, modifier: Modifier = Modifier, error: String? = null) {
    var picking by rememberSaveable { mutableStateOf(false) }
    PickerField(label, value, onClick = { picking = true }, modifier = modifier, error = error, tag = tag)
    if (picking) DayPickerDialog(iso, onPick = onPick, onDismiss = { picking = false })
}

/** A time field: its label ("14:50") opens the Material time picker at [minute] after midnight. */
@Composable
fun TimeField(label: String, value: String, minute: Int, onPick: (Int) -> Unit, tag: String, modifier: Modifier = Modifier, error: String? = null) {
    var picking by rememberSaveable { mutableStateOf(false) }
    PickerField(label, value, onClick = { picking = true }, modifier = modifier, error = error, tag = tag)
    if (picking) TimePickerDialog(label, minute, onPick = onPick, onDismiss = { picking = false })
}

/**
 * Material 3 time picker in a dialog (24 h, as the desktop). Hour and minute become minutes after
 * midnight: plain input, the core does every check.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TimePickerDialog(title: String, minute: Int, onPick: (Int) -> Unit, onDismiss: () -> Unit) {
    val state = rememberTimePickerState(initialHour = minute / 60, initialMinute = minute % 60, is24Hour = true)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { TimePicker(state) },
        confirmButton = {
            TextButton(
                onClick = {
                    onPick(state.hour * 60 + state.minute)
                    onDismiss()
                },
                modifier = Modifier.testTag("time-picker-ok"),
            ) { Text(stringResource(R.string.ok)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
