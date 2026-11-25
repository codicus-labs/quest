package app.quest.android.ui.task

import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SelectableDates
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import app.quest.android.R
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset
import java.time.format.DateTimeParseException

/**
 * Material's DatePicker counts days as UTC midnights in epoch ms; the core speaks ISO days.
 * Only this conversion lives in Kotlin; which day means what is the core's business.
 */
object PickerDays {
    /** Epoch ms of [iso]'s UTC midnight, or null when [iso] is not an ISO day. */
    fun toPickerMillis(iso: String?): Long? = try {
        iso?.let { LocalDate.parse(it).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli() }
    } catch (_: DateTimeParseException) {
        null
    }

    fun toIso(pickerMillis: Long): String = Instant.ofEpochMilli(pickerMillis).atZone(ZoneOffset.UTC).toLocalDate().toString()
}

/** Material 3 date picker dialog for an ISO day; [onPick] gets the chosen ISO day. Days after [latestIso] can't be picked. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DayPickerDialog(initialIso: String?, onPick: (String) -> Unit, onDismiss: () -> Unit, latestIso: String? = null) {
    val latest = PickerDays.toPickerMillis(latestIso)
    val selectable = remember(latest) {
        object : SelectableDates {
            override fun isSelectableDate(utcTimeMillis: Long) = latest == null || utcTimeMillis <= latest
        }
    }
    val state = rememberDatePickerState(initialSelectedDateMillis = PickerDays.toPickerMillis(initialIso), selectableDates = selectable)
    DatePickerDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(
                onClick = {
                    state.selectedDateMillis?.let { onPick(PickerDays.toIso(it)) }
                    onDismiss()
                },
                enabled = state.selectedDateMillis != null,
                modifier = Modifier.testTag("date-picker-ok"),
            ) { Text(stringResource(R.string.ok)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    ) {
        DatePicker(state)
    }
}
