package app.quest.android.ui.review

import androidx.compose.animation.core.animate
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.PeriodLabel
import app.quest.android.ui.components.WIDEST_WEEK
import app.quest.android.ui.task.Caps
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import kotlin.math.abs
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import uniffi.quest_mobile.ReviewData

class ReviewActions(
    val previous: () -> Unit,
    val next: () -> Unit,
    val thisWeek: () -> Unit,
    val selectWeek: (weekStart: String) -> Unit,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = ReviewActions({}, {}, {}, {})
    }
}

/** Where a swipe turns the week: a quarter of the width. */
private const val SWIPE_THRESHOLD = 0.25f

/**
 * The weekly review: the week by arrows or a swipe, its figures
 * and four charts. One column on phones, two from 840 dp.
 */
@Composable
fun ReviewScreen(state: ReviewUiState, actions: ReviewActions) {
    when (state) {
        ReviewUiState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is ReviewUiState.Failed -> Box(Modifier.fillMaxSize().padding(QuestSpacing.screen), contentAlignment = Alignment.Center) {
            Text(state.message, style = QuestType.body, color = QuestTheme.colors.textMuted)
        }
        is ReviewUiState.Ready -> ReviewPage(state.review, actions)
    }
}

@Composable
private fun ReviewPage(review: ReviewData, actions: ReviewActions) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val wide = maxWidth >= 840.dp
        SwipeWeeks(actions) {
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = QuestSpacing.screen).padding(bottom = QuestSpacing.section),
                verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
            ) {
                Header(review, actions)
                Figures(review, wide)
                if (wide) {
                    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.large)) {
                        ChartCard(stringResource(R.string.review_per_day), Modifier.weight(1.4f)) { DayBars(review.days) }
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(QuestSpacing.large)) {
                            ChartCard(stringResource(R.string.review_by_quest)) { QuestBars(review) }
                            ChartCard(stringResource(R.string.review_quest_vs_other)) { SplitBar(review) }
                        }
                    }
                } else {
                    ChartCard(stringResource(R.string.review_per_day)) { DayBars(review.days) }
                    ChartCard(stringResource(R.string.review_by_quest)) { QuestBars(review) }
                    ChartCard(stringResource(R.string.review_quest_vs_other)) { SplitBar(review) }
                }
                ChartCard(stringResource(R.string.review_recent_weeks)) { WeekBars(review, actions.selectWeek) }
            }
        }
    }
}

/** The page follows the finger sideways; past [SWIPE_THRESHOLD] it turns to the week before or after. */
@Composable
private fun SwipeWeeks(actions: ReviewActions, content: @Composable () -> Unit) {
    // Follows the finger synchronously; only settling back animates.
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
                    onDragStart = { settling?.cancel() },
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

/** Title and subtitle; the week with arrows, and "This week" when another one shows. */
@Composable
private fun Header(review: ReviewData, actions: ReviewActions) {
    val colors = QuestTheme.colors
    Column {
        Text(stringResource(R.string.review_title), style = QuestType.stat, color = colors.text, modifier = Modifier.semantics { heading() })
        Text(stringResource(R.string.review_subtitle), style = QuestType.body, color = colors.textMuted)
        FlowRow(itemVerticalAlignment = Alignment.CenterVertically) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = actions.previous, modifier = Modifier.testTag("review-previous")) {
                    Icon(Icons.AutoMirrored.Filled.KeyboardArrowLeft, contentDescription = stringResource(R.string.review_previous_week), tint = colors.textMuted)
                }
                PeriodLabel(review.weekLabel, WIDEST_WEEK, QuestType.bodyMedium, colors.text, textModifier = Modifier.testTag("review-week-label"))
                IconButton(onClick = actions.next, modifier = Modifier.testTag("review-next")) {
                    Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = stringResource(R.string.review_next_week), tint = colors.textMuted)
                }
            }
            if (!review.isThisWeek) {
                OutlinedButton(onClick = actions.thisWeek, contentPadding = PaddingValues(horizontal = QuestSpacing.medium), modifier = Modifier.testTag("review-this-week")) {
                    Text(stringResource(R.string.review_this_week), style = QuestType.bodyMedium)
                }
            }
        }
    }
}

/** Quest hours against the target (with progress), daily average, quest work's share. */
@Composable
private fun Figures(review: ReviewData, wide: Boolean) {
    val colors = QuestTheme.colors
    val questHours = @Composable { modifier: Modifier ->
        FigureCard(stringResource(R.string.review_quest_hours), modifier) {
            Row(verticalAlignment = Alignment.Bottom) {
                Text(review.questHours, style = QuestType.bigStat, color = colors.accent, modifier = Modifier.testTag("review-quest-hours"))
                Text(review.target, style = QuestType.body, color = colors.textMuted, modifier = Modifier.padding(start = QuestSpacing.tight, bottom = 4.dp))
            }
            LinearProgressIndicator(
                progress = { review.targetProgress.coerceIn(0f, 1f) },
                modifier = Modifier.fillMaxWidth().height(6.dp).clip(RoundedCornerShape(50)),
                color = colors.accent,
                trackColor = colors.barTrack,
                strokeCap = StrokeCap.Round,
                gapSize = 0.dp,
                drawStopIndicator = {},
            )
            Text(review.targetNote, style = QuestType.small, color = colors.textFaint)
        }
    }
    val average = @Composable { modifier: Modifier ->
        FigureCard(stringResource(R.string.review_daily_average), modifier) {
            Text(review.dailyAverage, style = QuestType.bigStat, color = colors.text, modifier = Modifier.testTag("review-daily-average"))
        }
    }
    val share = @Composable { modifier: Modifier ->
        FigureCard(stringResource(R.string.review_quest_vs_other), modifier) {
            Text(review.questShare, style = QuestType.bigStat, color = colors.text, modifier = Modifier.testTag("review-quest-share"))
        }
    }
    if (wide) {
        Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.large)) {
            questHours(Modifier.weight(1f))
            average(Modifier.weight(1f))
            share(Modifier.weight(1f))
        }
    } else {
        questHours(Modifier.fillMaxWidth())
        Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.large)) {
            average(Modifier.weight(1f))
            share(Modifier.weight(1f))
        }
    }
}

@Composable
private fun FigureCard(title: String, modifier: Modifier, content: @Composable () -> Unit) {
    Card(modifier.semantics(mergeDescendants = true) {}) {
        Caps(title)
        content()
    }
}

@Composable
private fun ChartCard(title: String, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Card(modifier.fillMaxWidth()) {
        Caps(title)
        content()
    }
}

@Composable
private fun Card(modifier: Modifier, content: @Composable () -> Unit) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    Column(
        modifier.background(colors.card, shape).border(QuestSpacing.hairline, colors.border, shape).padding(QuestSpacing.large),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) { content() }
}
