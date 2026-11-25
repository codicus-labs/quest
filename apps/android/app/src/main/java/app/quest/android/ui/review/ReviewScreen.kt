package app.quest.android.ui.review

import androidx.compose.animation.core.animate
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.PeriodLabel
import app.quest.android.ui.components.QuestBottomSheet
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.WIDEST_WEEK
import app.quest.android.ui.task.TaskCalendarChoices
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import kotlin.math.abs
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import uniffi.quest_mobile.CalendarMonth
import uniffi.quest_mobile.DateTarget
import uniffi.quest_mobile.QuickFields
import uniffi.quest_mobile.ReviewPostponed
import uniffi.quest_mobile.ReviewSince
import uniffi.quest_mobile.WeeklyReviewData

class ReviewActions(
    val previous: () -> Unit,
    val next: () -> Unit,
    val openTask: (String) -> Unit = {},
    val fields: (String) -> QuickFields? = { null },
    val calendar: (String?, QuickFields, DateTarget) -> CalendarMonth? = { _, _, _ -> null },
    val shiftMonth: (String, Boolean) -> String? = { _, _ -> null },
    val setDate: (String, DateTarget, String?) -> Unit = { _, _, _ -> },
    val moveToInbox: (String) -> Unit = {},
    val delete: (String) -> Unit = {},
) {
    companion object {
        val None = ReviewActions({}, {})
    }
}

private const val SWIPE_THRESHOLD = 0.25f

private data class DatePicker(val taskId: String, val target: DateTarget, val fields: QuickFields)

@Composable
fun ReviewScreen(state: ReviewUiState, actions: ReviewActions, footer: @Composable () -> Unit = {}) {
    Column(Modifier.fillMaxSize()) {
        Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
            when (state) {
                ReviewUiState.Loading -> CircularProgressIndicator()
                is ReviewUiState.Failed -> Text(state.message, style = QuestType.body, color = QuestTheme.colors.text)
                is ReviewUiState.Ready -> ReviewPage(state.review, actions)
            }
        }
        footer()
    }
}

@Composable
private fun ReviewPage(review: WeeklyReviewData, actions: ReviewActions) {
    var picker by remember { mutableStateOf<DatePicker?>(null) }
    SwipeWeeks(actions) {
        BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
            val wide = maxWidth >= 840.dp && LocalDensity.current.fontScale <= 1.3f
            val stackedMetrics = maxWidth < 380.dp || LocalDensity.current.fontScale > 1.3f
            Column(
                Modifier.widthIn(max = 1040.dp).fillMaxSize().verticalScroll(rememberScrollState())
                    .testTag("review-scroll").padding(horizontal = QuestSpacing.screen).padding(bottom = QuestSpacing.section),
                verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
            ) {
                WeekHeader(review.weekLabel, actions)
                ReviewMetrics(review, stackedMetrics)
                val ranked = @Composable {
                    ReviewSection(stringResource(R.string.review_most_postponed), empty = review.mostPostponed.isEmpty()) {
                        review.mostPostponed.forEach { task ->
                            PostponedRow(task, actions) { target ->
                                actions.fields(task.taskId)?.let { picker = DatePicker(task.taskId, target, it) }
                            }
                        }
                    }
                }
                if (wide) {
                    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.section)) {
                        Column(Modifier.weight(1f)) { ranked() }
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(QuestSpacing.large)) {
                            ReviewRemainder(review, actions)
                        }
                    }
                } else {
                    ranked()
                    ReviewRemainder(review, actions)
                }
            }
        }
    }
    picker?.let { active ->
        val title = if (active.target == DateTarget.PLAN) R.string.review_schedule else R.string.review_set_deadline
        QuestBottomSheet(stringResource(title), onDismiss = { picker = null }) {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                TaskCalendarChoices(active.target, { actions.calendar(it, active.fields, active.target) }, actions.shiftMonth) {
                    actions.setDate(active.taskId, active.target, it)
                    picker = null
                }
            }
        }
    }
}

@Composable
private fun SwipeWeeks(actions: ReviewActions, content: @Composable () -> Unit) {
    var offset by remember { mutableFloatStateOf(0f) }
    var settling by remember { mutableStateOf<Job?>(null) }
    val scope = rememberCoroutineScope()
    val currentActions by rememberUpdatedState(actions)
    fun settle(from: Float) {
        settling = scope.launch { animate(from, 0f) { value, _ -> offset = value } }
    }
    Box(
        Modifier
            .fillMaxSize()
            .pointerInput(Unit) {
                detectHorizontalDragGestures(
                    onDragStart = { settling?.cancel(); offset = 0f },
                    onDragEnd = {
                        val moved = offset
                        val turned = abs(moved) > size.width * SWIPE_THRESHOLD
                        if (turned) {
                            if (moved > 0) currentActions.previous() else currentActions.next()
                        }
                        settle(if (turned) -moved / 3 else moved)
                    },
                    onDragCancel = { settle(offset) },
                ) { change, dx ->
                    change.consume()
                    offset += dx
                }
            }
            .graphicsLayer { translationX = offset }
            .testTag("review-pages"),
    ) { content() }
}

@Composable
private fun WeekHeader(label: String, actions: ReviewActions) {
    val colors = QuestTheme.colors
    Row(verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = actions.previous, modifier = Modifier.testTag("review-previous")) {
            Icon(Icons.AutoMirrored.Filled.KeyboardArrowLeft, stringResource(R.string.review_previous_week), tint = colors.text)
        }
        PeriodLabel(label, WIDEST_WEEK, QuestType.bodyMedium, colors.text, Modifier.weight(1f, fill = false))
        IconButton(onClick = actions.next, modifier = Modifier.testTag("review-next")) {
            Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, stringResource(R.string.review_next_week), tint = colors.text)
        }
    }
}

@Composable
private fun ReviewRemainder(review: WeeklyReviewData, actions: ReviewActions) {
    StatusSection(R.string.review_carried_over, review.carriedOver, actions.openTask)
    StatusSection(R.string.review_overdue, review.overdue, actions.openTask, overdue = true)
    ReviewSection(stringResource(R.string.review_where_time)) {
        ReviewTime(review)
    }
}

@Composable
private fun ReviewSection(title: String, empty: Boolean = false, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Text(title, style = QuestType.nav, color = QuestTheme.colors.text, modifier = Modifier.semantics { heading() })
        if (empty) {
            Text(stringResource(R.string.review_empty), style = QuestType.small, color = QuestTheme.colors.textMuted)
        } else {
            content()
        }
    }
}

@Composable
private fun StatusSection(title: Int, tasks: List<ReviewSince>, open: (String) -> Unit, overdue: Boolean = false) {
    ReviewSection(stringResource(title), empty = tasks.isEmpty()) {
        tasks.forEach { task ->
            Column {
                TaskTitle(task.title, { open(task.taskId) })
                Text(
                    stringResource(R.string.review_since, reviewDate(task.since)),
                    style = QuestType.small,
                    color = if (overdue) QuestTheme.colors.error else QuestTheme.colors.textMuted,
                )
            }
            HorizontalDivider(color = QuestTheme.colors.border)
        }
    }
}

@Composable
private fun PostponedRow(task: ReviewPostponed, actions: ReviewActions, pickDate: (DateTarget) -> Unit) {
    val colors = QuestTheme.colors
    Column(Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
            Box(Modifier.weight(1f)) { TaskTitle(task.title, { actions.openTask(task.taskId) }) }
            Text("↷ ${task.count}", style = QuestType.smallMedium, color = colors.accent, modifier = Modifier.testTag("review-count:${task.taskId}"))
        }
        History(R.string.review_plan, task.plan)
        History(R.string.task_deadline, task.deadline)
        if (!task.isDone) {
            Row {
                IconButton(onClick = { pickDate(DateTarget.PLAN) }, modifier = Modifier.testTag("review-plan:${task.taskId}")) {
                    Icon(Icons.Filled.DateRange, stringResource(R.string.review_schedule), Modifier.size(20.dp), tint = colors.textMuted)
                }
                IconButton(onClick = { pickDate(DateTarget.DEADLINE) }) {
                    Icon(QuestIcons.Flag, stringResource(R.string.review_set_deadline), Modifier.size(20.dp), tint = colors.textMuted)
                }
                IconButton(onClick = { actions.moveToInbox(task.taskId) }) {
                    Icon(QuestIcons.Inbox, stringResource(R.string.review_move_to_inbox), Modifier.size(20.dp), tint = colors.textMuted)
                }
                IconButton(onClick = { actions.delete(task.taskId) }, modifier = Modifier.testTag("review-delete:${task.taskId}")) {
                    Icon(QuestIcons.Trash, stringResource(R.string.delete), Modifier.size(20.dp), tint = colors.textMuted)
                }
            }
        }
        HorizontalDivider(color = colors.border, modifier = Modifier.padding(top = QuestSpacing.tight))
    }
}

@Composable
private fun TaskTitle(title: String, open: () -> Unit) {
    Box(
        Modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget).clickable(role = Role.Button, onClick = open),
        contentAlignment = Alignment.CenterStart,
    ) {
        Text(title, style = QuestType.bodyMedium, color = QuestTheme.colors.text)
    }
}

@Composable
private fun History(label: Int, chains: List<List<String>>) {
    chains.forEach { chain ->
        Text(
            stringResource(R.string.review_history, stringResource(label), chain.joinToString(" → ") { reviewDate(it) }),
            style = QuestType.small,
            color = QuestTheme.colors.textMuted,
        )
    }
}

private fun reviewDate(iso: String) = LocalDate.parse(iso).format(DateTimeFormatter.ofPattern("MMM d"))
