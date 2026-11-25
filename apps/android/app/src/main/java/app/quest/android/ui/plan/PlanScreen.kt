package app.quest.android.ui.plan

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.core.PlanPage
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.lists.ListMaxWidth
import app.quest.android.ui.lists.ListSectionLabel
import app.quest.android.ui.lists.PlaceTask
import app.quest.android.ui.lists.RowActions
import app.quest.android.ui.lists.SortItem
import app.quest.android.ui.lists.SortableTaskList
import app.quest.android.ui.lists.sortItems
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.time.format.TextStyle
import java.util.Locale
import kotlinx.coroutines.launch
import uniffi.quest_mobile.HomeTask
import uniffi.quest_mobile.ListRow
import uniffi.quest_mobile.PlanDaySection
import uniffi.quest_mobile.PlanTaskList
import uniffi.quest_mobile.PlanWeek

internal class PlanActions(
    val rows: RowActions,
    val add: (String) -> Unit,
    val place: PlaceTask,
    val undo: () -> Unit,
    val dismissToast: () -> Unit,
)

@Composable
internal fun PlanScreen(state: PlanUiState, actions: PlanActions) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        when (state) {
            PlanUiState.Loading -> CircularProgressIndicator(Modifier.align(Alignment.Center))
            is PlanUiState.Failed -> Text(state.message, Modifier.align(Alignment.Center), color = QuestTheme.colors.textMuted)
            is PlanUiState.Ready -> {
                PlanContent(state.page, actions)
                state.page.today.toast?.let { toast ->
                    UndoSnackbar(
                        toast.text,
                        onUndo = actions.undo,
                        actionLabel = toast.actionLabel,
                        onDismiss = actions.dismissToast,
                        modifier = Modifier.align(Alignment.BottomCenter).widthIn(max = 560.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun PlanContent(page: PlanPage, actions: PlanActions) {
    val plan = page.sections
    val today = plan.days.firstOrNull()?.date ?: return
    val tomorrow = plan.days.getOrNull(1)?.date
    var showUnplanned by rememberSaveable { mutableStateOf(false) }
    val scroll = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val entries = buildList {
        fun daySection(day: PlanDaySection) {
            val isToday = day.date == today
            val list = if (isToday) PlanTaskList.Today else PlanTaskList.Day(day.date)
            add(SortItem.Decoration(day.date, list) { DayHeading(day.date, today, tomorrow, actions.add) })
            if (isToday) {
                page.today.sections.forEachIndexed { index, section ->
                    section.label?.let { add(SortItem.Decoration("today-label-$index", list) { ListSectionLabel(it) }) }
                    section.rows.forEach { add(SortItem.Row("today-${it.id}", list, it)) }
                }
            } else {
                day.tasks.forEach { add(SortItem.Row("${day.date}-${it.id}", list, it.toListRow())) }
            }
            day.due.forEach { marker ->
                add(SortItem.Decoration("due-${day.date}-${marker.taskId}", list) { DueMarker(marker.title) })
            }
        }
        plan.days.forEach(::daySection)
        for (month in plan.later) {
            add(SortItem.Decoration("month-${month.start}") {
                val date = LocalDate.parse(month.start)
                val pattern = if (date.year == LocalDate.parse(today).year) "MMMM" else "MMMM yyyy"
                val label = date.format(DateTimeFormatter.ofPattern(pattern))
                ListSectionLabel(if (month.partial) stringResource(R.string.plan_month_rest, label) else label)
            })
            month.days.forEach(::daySection)
        }
        add(SortItem.Decoration("not-planned") {
            Row(
                Modifier.fillMaxWidth().clickable(role = Role.Button) { showUnplanned = !showUnplanned }
                    .heightIn(min = 48.dp).padding(horizontal = QuestSpacing.small).testTag("not-planned"),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small),
            ) {
                Text(if (showUnplanned) "▾" else "▸", color = QuestTheme.colors.textMuted)
                Text(stringResource(R.string.plan_not_planned), style = QuestType.task, color = QuestTheme.colors.text)
                Text(plan.notPlanned.size.toString(), style = QuestType.small, color = QuestTheme.colors.textMuted)
            }
        })
        if (showUnplanned) plan.notPlanned.forEach { add(SortItem.Row("unplanned-${it.id}", null, it.toListRow())) }
    }

    Column(Modifier.widthIn(max = ListMaxWidth).fillMaxSize().padding(horizontal = QuestSpacing.medium)) {
        WeekStrip(plan.weeks, today) { date ->
            val index = entries.indexOfFirst { it.key == date }
            if (index >= 0) scope.launch { scroll.scrollToItem(index) }
        }
        HorizontalDivider(color = QuestTheme.colors.textMuted.copy(alpha = 0.22f))
        SortableTaskList(
            scroll, entries, actions.rows, actions.place,
            Modifier.weight(1f).fillMaxWidth().clipToBounds(), planToday = today,
        ) { placed, draw ->
            LazyColumn(
                Modifier.fillMaxSize().testTag("plan-sections"),
                state = scroll,
                contentPadding = PaddingValues(bottom = 88.dp),
            ) { sortItems(placed, draw) }
        }
    }
}

@Composable
private fun DayHeading(date: String, today: String, tomorrow: String?, add: (String) -> Unit) {
    val day = LocalDate.parse(date)
    val isToday = date == today
    val label = when (date) {
        today -> stringResource(R.string.plan_today)
        tomorrow -> stringResource(R.string.plan_tomorrow)
        else -> day.dayOfWeek.getDisplayName(TextStyle.FULL, Locale.getDefault())
    }
    val fullDate = day.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.FULL))
    val tint = if (isToday) QuestTheme.colors.accent else QuestTheme.colors.text
    val numberStyle = QuestType.heading.copy(fontFeatureSettings = "tnum")
    val numberWidth = rememberTextMeasurer(cacheSize = 1).measure("00", style = numberStyle).size.width
    val numberColumnWidth = with(LocalDensity.current) { numberWidth.toDp() }
    Row(
        Modifier.fillMaxWidth().testTag("plan-day-$date").padding(start = QuestSpacing.small, top = QuestSpacing.small),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        Row(
            Modifier.weight(1f).semantics {
                heading()
                contentDescription = if (isToday || date == tomorrow) "$label, $fullDate" else fullDate
            },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small),
        ) {
            Text(day.dayOfMonth.toString(), Modifier.width(numberColumnWidth), style = numberStyle, color = tint)
            Text(label, style = QuestType.task, color = tint, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        IconButton(onClick = { add(date) }, modifier = Modifier.testTag("plan-add-$date")) {
            Icon(Icons.Filled.Add, stringResource(R.string.plan_add_for_day, fullDate), tint = QuestTheme.colors.textMuted)
        }
    }
}

@Composable
private fun DueMarker(title: String) {
    Text(
        stringResource(R.string.plan_due_marker, title),
        style = QuestType.small,
        color = QuestTheme.colors.textMuted,
        modifier = Modifier.padding(start = 48.dp, top = 4.dp, bottom = 4.dp),
    )
}

@Composable
private fun WeekStrip(weeks: List<PlanWeek>, today: String, onJump: (String) -> Unit) {
    val pager = rememberPagerState { weeks.size }
    HorizontalPager(state = pager, modifier = Modifier.fillMaxWidth().testTag("plan-weeks")) { week ->
        Row(Modifier.fillMaxWidth()) {
            for (entry in weeks[week].days) {
                val day = LocalDate.parse(entry.date)
                val fullDate = day.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.FULL))
                val selected = entry.date == today
                val label = if (entry.hasTasks) stringResource(R.string.plan_week_has_tasks, fullDate) else fullDate
                val colors = QuestTheme.colors
                val tint = if (selected) colors.accent else colors.textMuted
                Column(
                    Modifier.weight(1f).heightIn(min = 52.dp)
                        .clickable(enabled = entry.jumpEnabled, role = Role.Button) { onJump(entry.date) }
                        .semantics { contentDescription = label; this.selected = selected }
                        .testTag("plan-week-${entry.date}"),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    Text(day.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault()), style = QuestType.small, color = tint, maxLines = 1)
                    Text(day.dayOfMonth.toString(), style = QuestType.task, color = if (selected) tint else colors.text)
                    Box(Modifier.padding(top = 2.dp).size(4.dp).then(if (entry.hasTasks) Modifier.background(tint, CircleShape) else Modifier))
                }
            }
        }
    }
}

private fun HomeTask.toListRow() = ListRow(
    id = id, title = title, done = done, quest = quest, marks = emptyList(),
    timerRunning = timerRunning, timer = tracked, canStartTimer = canStartTimer,
)
