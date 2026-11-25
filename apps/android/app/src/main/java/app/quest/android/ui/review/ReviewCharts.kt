package app.quest.android.ui.review

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.max
import app.quest.android.R
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.ReviewData
import uniffi.quest_mobile.ReviewDay
import uniffi.quest_mobile.ReviewQuest
import uniffi.quest_mobile.ReviewWeek

// Every chart draws the core's fractions (0..1) times the space it has; Kotlin computes no numbers.

private val BarArea = 160.dp

/** Quest hours per day: a column per day, stacked by quest from the bottom, its hours above. */
@Composable
fun DayBars(days: List<ReviewDay>) {
    val colors = QuestTheme.colors
    Row(Modifier.fillMaxWidth().testTag("review-days"), horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        days.forEach { day ->
            val highlight = if (day.isToday) colors.accent else colors.textFaint
            Column(
                Modifier.weight(1f).testTag("review-day:${day.label}").clearAndSetSemantics { contentDescription = day.a11yLabel },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(QuestSpacing.tight),
            ) {
                Text(day.hours, style = QuestType.small, color = highlight, maxLines = 1)
                Canvas(Modifier.fillMaxWidth(0.7f).height(BarArea)) {
                    val radius = CornerRadius(3.dp.toPx())
                    // An empty day: a short line on the baseline, as on the desktop.
                    if (day.segments.isEmpty() || day.fraction <= 0f) {
                        drawRoundRect(colors.barIdle, Offset(0f, size.height - 3.dp.toPx()), Size(size.width, 3.dp.toPx()), radius)
                        return@Canvas
                    }
                    var bottom = size.height
                    day.segments.forEach { segment ->
                        val height = size.height * segment.fraction.coerceIn(0f, 1f)
                        drawRoundRect(questColor(segment.colorHex), Offset(0f, bottom - height), Size(size.width, height), radius)
                        bottom -= height
                    }
                }
                Text(day.label, style = QuestType.small, color = highlight, maxLines = 1)
            }
        }
    }
}

/** Hours by quest: name, a bar relative to the largest, its hours. TalkBack reads the core's summary first. */
@Composable
fun QuestBars(review: ReviewData) {
    val quests = review.byQuest
    if (quests.isEmpty()) {
        Text(stringResource(R.string.review_no_quest_time), style = QuestType.body, color = QuestTheme.colors.textMuted, modifier = Modifier.testTag("review-no-quest-time"))
        return
    }
    Column(
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
        modifier = Modifier.semantics { contentDescription = review.byQuestA11y }.testTag("review-by-quest"),
    ) {
        quests.forEach { quest -> QuestBar(quest) }
    }
}

@Composable
private fun QuestBar(quest: ReviewQuest) {
    val colors = QuestTheme.colors
    val color = questColor(quest.colorHex)
    Row(
        Modifier.fillMaxWidth().heightIn(min = 24.dp).clearAndSetSemantics { contentDescription = quest.a11yLabel },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        Dot(color, 8.dp)
        Text(quest.name, style = QuestType.body, color = colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        Canvas(Modifier.weight(1f).height(8.dp)) {
            val radius = CornerRadius(size.height / 2)
            drawRoundRect(colors.barTrack, cornerRadius = radius)
            drawRoundRect(color, size = Size(size.width * quest.fraction.coerceIn(0f, 1f), size.height), cornerRadius = radius)
        }
        Text(quest.hours, style = QuestType.body, color = colors.text, maxLines = 1)
    }
}

/** Quest work vs other: one bar split by the core's fraction, with its legend. */
@Composable
fun SplitBar(review: ReviewData) {
    val colors = QuestTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
        Canvas(Modifier.fillMaxWidth().height(28.dp).testTag("review-split").clearAndSetSemantics { contentDescription = review.splitA11y }) {
            val radius = CornerRadius(6.dp.toPx())
            drawRoundRect(colors.barTrack, cornerRadius = radius)
            val quest = size.width * review.questFraction.coerceIn(0f, 1f)
            if (quest > 0f) drawRoundRect(colors.accent, size = Size(quest, size.height), cornerRadius = radius)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.large), verticalAlignment = Alignment.CenterVertically) {
            Legend(review.questWork, colors.accent)
            Legend(review.other, colors.barIdle)
        }
    }
}

@Composable
private fun Legend(label: String, color: androidx.compose.ui.graphics.Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Dot(color, 10.dp)
        Text(label, style = QuestType.body, color = QuestTheme.colors.text)
    }
}

/**
 * The recent weeks' Quest hours; the shown week stands out, a tap shows another. TalkBack reads the
 * core's summary on the chart and can still pick each week (its own button, "selected" for the shown one).
 */
@Composable
fun WeekBars(review: ReviewData, onSelect: (String) -> Unit) {
    val selectLabel = stringResource(R.string.review_select_week)
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val count = review.weeks.size.coerceAtLeast(1)
        val fitted = (maxWidth - QuestSpacing.small * (count - 1)) / count
        // Each bar stays a 48 dp touch target; narrow screens scroll, starting at the newest week.
        val scroll = rememberScrollState()
        LaunchedEffect(scroll.maxValue) { scroll.scrollTo(scroll.maxValue) }
        Row(
            Modifier
                .fillMaxWidth()
                .horizontalScroll(scroll)
                .semantics { contentDescription = review.weeksA11y }
                .testTag("review-weeks"),
            horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small),
        ) {
            review.weeks.forEach { week -> WeekBar(week, selectLabel, onSelect, Modifier.width(max(48.dp, fitted))) }
        }
    }
}

@Composable
private fun WeekBar(week: ReviewWeek, selectLabel: String, onSelect: (String) -> Unit, modifier: Modifier) {
    val colors = QuestTheme.colors
    val color = if (week.selected) colors.accent else colors.barIdle
    Column(
        modifier
            .clip(RoundedCornerShape(QuestSpacing.fieldRadius))
            .clickable(onClickLabel = selectLabel) { onSelect(week.weekStart) }
            .testTag("review-week:${week.weekStart}")
            .clearAndSetSemantics {
                contentDescription = week.a11yLabel
                role = Role.Button
                selected = week.selected
                onClick(selectLabel) {
                    onSelect(week.weekStart)
                    true
                }
            },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(week.hours, style = QuestType.small, color = if (week.selected) colors.text else colors.textFaint, maxLines = 1)
        Box(Modifier.fillMaxWidth(0.6f).height(96.dp), contentAlignment = Alignment.BottomCenter) {
            val fraction = week.fraction.coerceIn(0f, 1f)
            Box(
                Modifier
                    .fillMaxWidth()
                    .then(if (fraction > 0f) Modifier.fillMaxHeight(fraction) else Modifier.height(3.dp))
                    .background(color, RoundedCornerShape(3.dp)),
            )
        }
        Text(week.label, style = QuestType.small, color = if (week.selected) colors.text else colors.textFaint, maxLines = 1)
    }
}
