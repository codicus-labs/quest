package app.quest.android.ui.review

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import java.text.NumberFormat
import java.time.DayOfWeek
import java.time.format.TextStyle
import java.util.Locale
import uniffi.quest_mobile.ReviewBucket
import uniffi.quest_mobile.ReviewShare
import uniffi.quest_mobile.WeeklyReviewData

@Composable
internal fun ReviewMetrics(review: WeeklyReviewData, stacked: Boolean) {
    val colors = QuestTheme.colors
    val current = review.current
    val previous = review.previous
    val figures = listOf(
        Triple(R.string.review_done_added, "${current.done} / ${current.added}", "${previous.done} / ${previous.added}"),
        Triple(R.string.review_quest_goal, "${hours(current.questMs)} / ${hours(review.goalMs)}", hours(previous.questMs)),
        Triple(R.string.review_postponements, current.postponements.toString(), previous.postponements.toString()),
    )
    val content = @Composable { (label, value, prior): Triple<Int, String, String>, modifier: Modifier ->
        Column(modifier, verticalArrangement = Arrangement.spacedBy(QuestSpacing.tight)) {
            Text(stringResource(label), style = QuestType.small, color = colors.textMuted)
            Text(value, style = QuestType.nav, color = colors.text)
            Text(stringResource(R.string.review_previous_value, prior), style = QuestType.small, color = colors.textMuted)
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
        HorizontalDivider(color = colors.border)
        if (stacked) {
            figures.forEach { content(it, Modifier.fillMaxWidth()) }
        } else {
            Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
                figures.forEach { content(it, Modifier.weight(1f)) }
            }
        }
        HorizontalDivider(color = colors.border)
    }
}

@Composable
internal fun ReviewTime(review: WeeklyReviewData) {
    DayBars(review.trackedMsPerDay)
    review.shares.forEach { TimeShare(it) }
}

@Composable
private fun DayBars(days: List<Long>) {
    val max = (days.maxOrNull() ?: 0).coerceAtLeast(1)
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(3.dp)
    BoxWithConstraints {
        if (maxWidth < 315.dp || LocalDensity.current.fontScale > 1.3f) {
            Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
                days.forEachIndexed { index, ms ->
                    Text("${dayName(index)} · ${hours(ms)}", style = QuestType.small, color = colors.text)
                    ShareBar(ms.toFloat() / max, colors.accent)
                }
            }
        } else {
            Row(Modifier.fillMaxWidth().padding(vertical = QuestSpacing.small), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                days.forEachIndexed { index, ms ->
                    val summary = "${dayName(index, TextStyle.FULL)}: ${hours(ms)}"
                    Column(
                        Modifier.weight(1f).clearAndSetSemantics { contentDescription = summary },
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Box(Modifier.fillMaxWidth(0.6f).height(72.dp), contentAlignment = Alignment.BottomCenter) {
                            Box(
                                Modifier.fillMaxWidth().then(
                                    if (ms == 0L) Modifier.height(2.dp) else Modifier.fillMaxHeight(ms.toFloat() / max),
                                ).background(if (ms == 0L) colors.barTrack else colors.accent, shape),
                            )
                        }
                        Text(dayName(index), style = QuestType.small, color = colors.text)
                        Text(hours(ms), style = QuestType.small, color = colors.textMuted)
                    }
                }
            }
        }
    }
}

@Composable
private fun TimeShare(share: ReviewShare) {
    val colors = QuestTheme.colors
    val (title, color) = when (val bucket = share.bucket) {
        is ReviewBucket.Quest -> bucket.title to questColor(bucket.colorHex)
        ReviewBucket.Organizing -> stringResource(R.string.organizing) to colors.textMuted
        ReviewBucket.NoQuest -> stringResource(R.string.no_quest) to colors.textMuted
    }
    val fraction = share.fraction.coerceIn(0.0, 1.0)
    val percent = NumberFormat.getPercentInstance().format(fraction)
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.tight)) {
        Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
            Text(title, modifier = Modifier.weight(1f), color = colors.text, style = QuestType.body)
            Text("${hours(share.trackedMs)} · $percent", color = colors.textMuted, style = QuestType.small)
        }
        ShareBar(fraction.toFloat(), color)
    }
}

@Composable
private fun ShareBar(fraction: Float, color: Color) {
    val shape = RoundedCornerShape(50)
    Box(Modifier.fillMaxWidth().height(5.dp).background(QuestTheme.colors.barTrack, shape)) {
        Box(Modifier.fillMaxWidth(fraction).fillMaxHeight().background(color, shape))
    }
}

private fun dayName(index: Int, style: TextStyle = TextStyle.SHORT): String =
    DayOfWeek.of(index + 1).getDisplayName(style, Locale.getDefault())

private fun hours(ms: Long): String = NumberFormat.getNumberInstance().apply { maximumFractionDigits = 1 }
    .format(ms / 3_600_000.0) + "h"
