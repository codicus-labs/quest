package app.quest.android.ui.calendar

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.PeriodLabel
import app.quest.android.ui.components.WIDEST_MONTH
import app.quest.android.ui.components.WIDEST_WEEK
import app.quest.android.ui.components.HourLabelWidth
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import kotlin.math.abs
import kotlinx.coroutines.launch
import uniffi.quest_mobile.CalendarData
import uniffi.quest_mobile.CalendarDay

/** Everything the calendar screen does (the sheets have their own [CalendarSheetActions]). */
class CalendarActions(
    val previous: () -> Unit,
    val next: () -> Unit,
    val today: () -> Unit,
    val openLogWork: () -> Unit,
    val undo: () -> Unit,
    val dismissToast: () -> Unit,
    val grid: CalendarGridActions,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = CalendarActions({}, {}, {}, {}, {}, {}, CalendarGridActions({ _, _, _ -> null }, { _, _, _ -> }, {}))
    }
}

/** Days side by side for a content [width]: one on phones, three on medium, a week on expanded windows. */
fun calendarDayCount(width: Dp): Int = when {
    width < 600.dp -> 1
    width < 840.dp -> 3
    else -> 7
}

/** Where a swipe turns the page: a quarter of the width. */
private const val SWIPE_THRESHOLD = 0.25f

/**
 * The calendar: a page of days (1, 3 or 7) over 24 hours, swiped to the page before or after.
 * Header with the week's Quest Hours; each day's total under its name; Log work as a FAB (phones)
 * or in the header. Opens scrolled to the current hour.
 */
@Composable
fun CalendarScreen(
    state: CalendarUiState,
    actions: CalendarActions,
    dragState: CalendarDragState = remember { CalendarDragState() },
    scroll: ScrollState = rememberScrollState(),
) {
    when (state) {
        CalendarUiState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is CalendarUiState.Failed -> Box(Modifier.fillMaxSize().padding(QuestSpacing.screen), contentAlignment = Alignment.Center) {
            Text(state.message, style = QuestType.body, color = QuestTheme.colors.textMuted)
        }
        is CalendarUiState.Ready -> CalendarPage(state.calendar, actions, dragState, scroll)
    }
}

@Composable
private fun CalendarPage(calendar: CalendarData, actions: CalendarActions, dragState: CalendarDragState, scroll: ScrollState) {
    val hourHeight = 52.dp
    val density = LocalDensity.current
    // Once per visit (not after rotation): scroll to the current hour.
    var scrolledOnOpen by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        if (scrolledOnOpen) return@LaunchedEffect
        scrolledOnOpen = true
        scroll.scrollTo(with(density) { (hourHeight * calendar.scrollMinute.toFloat() / 60f).roundToPx() })
    }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val compact = calendar.days.size == 1
        Column(Modifier.fillMaxSize()) {
            CalendarHeader(calendar, actions, compact)
            SwipePages(actions, Modifier.weight(1f).padding(horizontal = if (compact) 0.dp else QuestSpacing.screen)) {
                DayHeads(calendar.days)
                CalendarGrid(calendar.days, scroll, dragState, actions.grid, Modifier.weight(1f).fillMaxWidth(), hourHeight)
            }
        }
        Column(
            Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(QuestSpacing.large),
            horizontalAlignment = Alignment.End,
            verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
        ) {
            calendar.toast?.let { toast ->
                UndoSnackbar(
                    toast.text,
                    onUndo = actions.undo,
                    actionLabel = toast.actionLabel,
                    onDismiss = actions.dismissToast,
                    modifier = Modifier.align(Alignment.CenterHorizontally).widthIn(max = 560.dp),
                )
            }
            if (compact) LogWorkFab(actions.openLogWork)
        }
    }
}

@Composable
private fun LogWorkFab(onClick: () -> Unit) {
    ExtendedFloatingActionButton(
        onClick = onClick,
        icon = { Icon(Icons.Filled.Add, contentDescription = null) },
        text = { Text(stringResource(R.string.log_work)) },
        containerColor = QuestTheme.colors.accent,
        contentColor = QuestTheme.colors.card,
        modifier = Modifier.testTag("calendar-log-work"),
    )
}

/**
 * The page follows the finger sideways; let go past [SWIPE_THRESHOLD] and it turns to the page
 * before or after. Vertical scrolling and the grid's long-press drags take their own gestures first.
 */
@Composable
private fun SwipePages(actions: CalendarActions, modifier: Modifier, content: @Composable () -> Unit) {
    val offset = remember { Animatable(0f) }
    val scope = rememberCoroutineScope()
    val currentActions by rememberUpdatedState(actions)
    Column(
        modifier
            .pointerInput(Unit) {
                val width = size.width.toFloat()
                detectHorizontalDragGestures(
                    onDragEnd = {
                        val moved = offset.value
                        scope.launch {
                            if (abs(moved) > width * SWIPE_THRESHOLD) {
                                if (moved > 0) currentActions.previous() else currentActions.next()
                                offset.snapTo(-moved / 3)
                            }
                            offset.animateTo(0f)
                        }
                    },
                    onDragCancel = { scope.launch { offset.animateTo(0f) } },
                ) { change, dx ->
                    change.consume()
                    scope.launch { offset.snapTo(offset.value + dx) }
                }
            }
            .graphicsLayer { translationX = offset.value }
            .testTag("calendar-pages"),
    ) { content() }
}

/** Phones: month, arrows, Today; wider: the week, arrows, Today and Log work. Then the week's Quest Hours. */
@Composable
private fun CalendarHeader(calendar: CalendarData, actions: CalendarActions, compact: Boolean) {
    val colors = QuestTheme.colors
    val week = calendar.days.size >= 7
    Column(Modifier.fillMaxWidth().padding(start = QuestSpacing.screen, end = QuestSpacing.small, bottom = QuestSpacing.small)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            // The title is as wide as its widest month or week, so the arrows after it stay put; the rest goes right.
            Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically) {
                PeriodLabel(
                    if (compact) calendar.monthLabel else calendar.weekLabel,
                    if (compact) WIDEST_MONTH else WIDEST_WEEK,
                    QuestType.stat,
                    colors.text,
                    modifier = Modifier.weight(1f, fill = false),
                    textModifier = Modifier.semantics { heading() }.testTag("calendar-title"),
                    alignment = Alignment.CenterStart,
                )
                IconButton(onClick = actions.previous, modifier = Modifier.testTag("calendar-previous")) {
                    Icon(
                        Icons.AutoMirrored.Filled.KeyboardArrowLeft,
                        contentDescription = stringResource(if (week) R.string.calendar_previous_week else R.string.calendar_previous_days),
                        tint = colors.textMuted,
                    )
                }
                IconButton(onClick = actions.next, modifier = Modifier.testTag("calendar-next")) {
                    Icon(
                        Icons.AutoMirrored.Filled.KeyboardArrowRight,
                        contentDescription = stringResource(if (week) R.string.calendar_next_week else R.string.calendar_next_days),
                        tint = colors.textMuted,
                    )
                }
            }
            if (!calendar.showsToday) {
                OutlinedButton(
                    onClick = actions.today,
                    contentPadding = PaddingValues(horizontal = QuestSpacing.medium),
                    modifier = Modifier.padding(end = QuestSpacing.small).testTag("calendar-today"),
                ) { Text(stringResource(R.string.plan_today), style = QuestType.bodyMedium) }
            }
            if (!compact) {
                OutlinedButton(
                    onClick = actions.openLogWork,
                    contentPadding = PaddingValues(horizontal = QuestSpacing.medium),
                    modifier = Modifier.padding(end = QuestSpacing.medium).testTag("calendar-log-work"),
                ) { Text(stringResource(R.string.log_work), style = QuestType.bodyMedium) }
                WeekHours(calendar, Modifier.padding(end = QuestSpacing.medium))
            }
        }
        if (compact) {
            // Wraps the hours under the week at large font sizes instead of squeezing the week.
            FlowRow(itemVerticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(end = QuestSpacing.medium)) {
                Text(calendar.weekLabel, style = QuestType.small, color = colors.textMuted)
                Box(Modifier.weight(1f).widthIn(min = QuestSpacing.medium))
                WeekHours(calendar)
            }
        }
    }
}

/** "12.5h / 20h quest" and the week's progress bar (desktop Calendar header). */
@Composable
private fun WeekHours(calendar: CalendarData, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(calendar.weekHours, style = QuestType.bodyMedium, color = colors.text, modifier = Modifier.testTag("calendar-week-hours"))
            Text(stringResource(R.string.calendar_of_target, calendar.weeklyTarget), style = QuestType.small, color = colors.textMuted)
        }
        LinearProgressIndicator(
            progress = { calendar.weekProgress.coerceIn(0f, 1f) },
            modifier = Modifier.width(96.dp).height(6.dp).clip(RoundedCornerShape(50)).testTag("calendar-week-progress"),
            color = colors.accent,
            trackColor = colors.barTrack,
            strokeCap = StrokeCap.Round,
            gapSize = 0.dp,
            drawStopIndicator = {},
        )
    }
}

/** Each day's name and number (today in the accent colour) with its tracked total. */
@Composable
private fun DayHeads(days: List<CalendarDay>) {
    val colors = QuestTheme.colors
    Row(Modifier.fillMaxWidth().background(colors.card).padding(start = HourLabelWidth, top = QuestSpacing.tight, bottom = QuestSpacing.tight)) {
        days.forEach { day ->
            Column(Modifier.weight(1f).testTag("calendar-head:${day.grid.day}"), horizontalAlignment = Alignment.CenterHorizontally) {
                Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.tight), verticalAlignment = Alignment.Bottom) {
                    Text(day.weekday, style = QuestType.bodyMedium, color = if (day.isToday) colors.text else colors.textMuted)
                    Text(day.dayNumber, style = QuestType.body, color = if (day.isToday) colors.accent else colors.textFaint)
                }
                Text(day.total, style = QuestType.small, color = colors.textFaint, modifier = Modifier.testTag("calendar-total:${day.grid.day}"))
            }
        }
    }
}
