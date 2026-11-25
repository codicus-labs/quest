package app.quest.android.ui.task

import androidx.compose.ui.semantics.heading
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import app.quest.android.ui.theme.StatusColors
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.RepeatFrequency
import uniffi.quest_mobile.RepeatPinChoice
import uniffi.quest_mobile.RepeatWeekday

/** What the repeat editor can do; the task sheet and the AI capture's proposal each wire it to their own form. */
class RepeatEditorActions(
    val edit: ((RepeatForm) -> RepeatForm) -> Unit,
    val toggleWeekday: (RepeatWeekday) -> Unit,
    val save: () -> Unit,
    /** "Stop repeating" (shown only when the task repeats). */
    val stop: () -> Unit,
    val close: () -> Unit,
)

/** The desktop's repeat dialog as a sheet page: every field, the core's live check, and its messages. */
@Composable
fun RepeatEditor(editor: RepeatEditorState, taskTitle: String, repeats: Boolean, actions: RepeatEditorActions) {
    val form = editor.form
    val edit = actions.edit
    Row(verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = actions.close, modifier = Modifier.testTag("repeat-back")) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back))
        }
        Column {
            Text(stringResource(R.string.repeat_title), style = QuestType.stat, color = QuestTheme.colors.text, modifier = Modifier.semantics { heading() })
            Text(taskTitle, style = QuestType.body, color = QuestTheme.colors.textMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
    Setting(R.string.repeat_every) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
            OutlinedTextField(
                value = form.every,
                onValueChange = { value -> edit { it.copy(every = value) } },
                label = { Text(stringResource(R.string.repeat_interval)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.width(96.dp).testTag("repeat-every"),
            )
            Choices(RepeatFrequency.entries, form.frequency, ::frequencyLabel, "repeat-frequency") { value -> edit { it.copy(frequency = value) } }
        }
    }
    when (form.frequency) {
        RepeatFrequency.WEEK -> Setting(R.string.repeat_on) { Weekdays(editor.check.shownWeekdays, actions.toggleWeekday) }
        RepeatFrequency.MONTH -> Setting(R.string.repeat_on) { MonthDay(editor, edit) }
        else -> Unit
    }
    Setting(R.string.repeat_pin) {
        Choices(RepeatPinChoice.entries, form.pin, ::pinLabel, "repeat-pin") { value -> edit { it.copy(pin = value) } }
    }
    Setting(R.string.repeat_starts) {
        DayField(editor.check.starts, R.string.repeat_start_date, R.string.repeat_choose_start, "repeat-start") { iso -> edit { it.copy(starts = iso) } }
    }
    Setting(R.string.repeat_ends) {
        Choices(listOf(false, true), form.endsOnDate, { if (it) R.string.repeat_ends_on_date else R.string.repeat_ends_never }, "repeat-ends") { value ->
            edit { it.copy(endsOnDate = value) }
        }
    }
    if (form.endsOnDate) {
        Setting(R.string.repeat_end_date) {
            DayField(editor.check.ends, R.string.repeat_end_date, R.string.repeat_choose_end, "repeat-end") { iso -> edit { it.copy(ends = iso) } }
        }
    }
    Setting(R.string.repeat_timezone) {
        OutlinedTextField(
            value = form.timeZone,
            onValueChange = { value -> edit { it.copy(timeZone = value) } },
            label = { Text(stringResource(R.string.repeat_timezone)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().testTag("repeat-timezone"),
        )
    }
    if (editor.check.summary.isNotEmpty()) {
        Text(editor.check.summary, style = QuestType.bodyMedium, color = QuestTheme.colors.text, modifier = Modifier.testTag("repeat-summary"))
    }
    editor.error?.let { Text(it, style = QuestType.body, color = StatusColors.failed, modifier = Modifier.testTag("repeat-error")) }
    Row(verticalAlignment = Alignment.CenterVertically) {
        if (repeats) {
            TextButton(onClick = actions.stop, modifier = Modifier.testTag("repeat-stop")) { Text(stringResource(R.string.repeat_stop)) }
        }
        Box(Modifier.weight(1f))
        TextButton(onClick = actions.close, modifier = Modifier.testTag("repeat-cancel")) { Text(stringResource(R.string.cancel)) }
        Button(onClick = actions.save, modifier = Modifier.padding(start = QuestSpacing.small).testTag("repeat-save")) {
            Text(stringResource(R.string.save))
        }
    }
}

private fun frequencyLabel(frequency: RepeatFrequency) = when (frequency) {
    RepeatFrequency.DAY -> R.string.repeat_day
    RepeatFrequency.WEEK -> R.string.repeat_week
    RepeatFrequency.MONTH -> R.string.repeat_month
    RepeatFrequency.YEAR -> R.string.repeat_year
}

private fun pinLabel(pin: RepeatPinChoice) = when (pin) {
    RepeatPinChoice.NONE -> R.string.repeat_pin_none
    RepeatPinChoice.TOP -> R.string.repeat_pin_top
    RepeatPinChoice.BOTTOM -> R.string.repeat_pin_bottom
}

/** A labelled row of the dialog ("Every", "Pin", …). */
@Composable
private fun Setting(label: Int, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.tight)) {
        Caps(stringResource(label))
        content()
    }
}

/** One of a few options, as a segmented button row. */
@Composable
private fun <T> Choices(options: List<T>, selected: T, label: (T) -> Int, tag: String, onSelect: (T) -> Unit) {
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().testTag(tag)) {
        options.forEachIndexed { index, option ->
            SegmentedButton(
                selected = option == selected,
                onClick = { onSelect(option) },
                shape = SegmentedButtonDefaults.itemShape(index, options.size),
                icon = {},
                modifier = Modifier.testTag("$tag-$index"),
            ) { Text(stringResource(label(option)), maxLines = 1) }
        }
    }
}

private val weekdayLabels = mapOf(
    RepeatWeekday.MON to (R.string.weekday_mo to R.string.weekday_monday),
    RepeatWeekday.TUE to (R.string.weekday_tu to R.string.weekday_tuesday),
    RepeatWeekday.WED to (R.string.weekday_we to R.string.weekday_wednesday),
    RepeatWeekday.THU to (R.string.weekday_th to R.string.weekday_thursday),
    RepeatWeekday.FRI to (R.string.weekday_fr to R.string.weekday_friday),
    RepeatWeekday.SAT to (R.string.weekday_sa to R.string.weekday_saturday),
    RepeatWeekday.SUN to (R.string.weekday_su to R.string.weekday_sunday),
)

/** Weekday chips Mo–Su; the core decides what toggling means (the last day stays on). */
@Composable
private fun Weekdays(shown: List<RepeatWeekday>, onToggle: (RepeatWeekday) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.tight)) {
        RepeatWeekday.entries.forEach { day ->
            val (short, long) = weekdayLabels.getValue(day)
            val name = stringResource(long)
            FilterChip(
                selected = day in shown,
                onClick = { onToggle(day) },
                label = { Text(stringResource(short)) },
                modifier = Modifier.semantics { contentDescription = name }.testTag("weekday-${day.name.lowercase()}"),
            )
        }
    }
}

@Composable
private fun MonthDay(editor: RepeatEditorState, edit: ((RepeatForm) -> RepeatForm) -> Unit) {
    val form = editor.form
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Choices(listOf(false, true), form.lastDay, { if (it) R.string.repeat_last_day else R.string.repeat_day_of_month }, "repeat-monthly") { value ->
            edit { it.copy(lastDay = value) }
        }
        if (!form.lastDay) {
            // The placeholder (the start's day, as on the desktop) shows once the empty field has focus.
            OutlinedTextField(
                value = form.monthDay,
                onValueChange = { value -> edit { it.copy(monthDay = value) } },
                label = { Text(stringResource(R.string.repeat_month_day)) },
                placeholder = { Text(editor.check.monthDayPlaceholder) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.width(96.dp).testTag("repeat-month-day"),
            )
        }
    }
}

/**
 * A day as the core shows it ("Nov 24, 2026"); tapping the field or its icon opens the date
 * picker, which hands back an ISO day. Phones pick dates rather than type them.
 */
@Composable
private fun DayField(day: DayValue?, label: Int, pickLabel: Int, tag: String, onPick: (String) -> Unit) {
    var picking by rememberSaveable { mutableStateOf(false) }
    val press = remember { MutableInteractionSource() }
    val pressed by press.collectIsPressedAsState()
    LaunchedEffect(pressed) { if (pressed) picking = true }
    OutlinedTextField(
        value = day?.label.orEmpty(),
        onValueChange = {},
        readOnly = true,
        label = { Text(stringResource(label)) },
        singleLine = true,
        interactionSource = press,
        trailingIcon = {
            IconButton(onClick = { picking = true }, modifier = Modifier.testTag("$tag-pick")) {
                Icon(Icons.Filled.DateRange, contentDescription = stringResource(pickLabel))
            }
        },
        modifier = Modifier.fillMaxWidth().testTag(tag),
    )
    if (picking) DayPickerDialog(day?.iso, onPick = onPick, onDismiss = { picking = false })
}
