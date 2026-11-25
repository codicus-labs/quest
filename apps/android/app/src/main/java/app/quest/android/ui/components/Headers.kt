package app.quest.android.ui.components

import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.heading
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import app.quest.android.R
import androidx.compose.ui.unit.dp
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType

/** The day, Quest Hours today and the week's progress towards the weekly target. */
@Composable
fun HoursHeader(
    dayLabel: String,
    todayHours: String,
    weekHours: String,
    weekProgress: Float,
    modifier: Modifier = Modifier,
    weeklyTarget: String? = null,
    /** Phones: a smaller date and less room, so the first cards fit on screen. */
    compact: Boolean = false,
    /** Next to the date (phones: the list / day grid switch). */
    trailing: @Composable () -> Unit = {},
) {
    val colors = QuestTheme.colors
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(if (compact) QuestSpacing.small else QuestSpacing.large)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                dayLabel,
                style = if (compact) QuestType.stat else QuestType.heading,
                color = colors.text,
                modifier = Modifier.weight(1f).semantics { heading() }.testTag("home-date"),
            )
            trailing()
        }
        // Large text moves the week's progress below today's hours instead of squeezing both.
        FlowRow(itemVerticalAlignment = Alignment.Bottom, verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
            Row(verticalAlignment = Alignment.Bottom) {
                Text(todayHours, style = QuestType.bigStat, color = colors.text, modifier = Modifier.testTag("today-hours"))
                Text(
                    stringResource(R.string.home_today),
                    style = QuestType.body,
                    color = colors.textMuted,
                    modifier = Modifier.padding(start = QuestSpacing.small, bottom = 4.dp),
                )
            }
            Box(Modifier.weight(1f).widthIn(min = QuestSpacing.large))
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
                LinearProgressIndicator(
                    progress = { weekProgress.coerceIn(0f, 1f) },
                    modifier = Modifier.width(140.dp).height(6.dp).clip(RoundedCornerShape(50)).testTag("week-progress"),
                    color = colors.accent,
                    trackColor = colors.barTrack,
                    strokeCap = StrokeCap.Round,
                    gapSize = 0.dp,
                    drawStopIndicator = {},
                )
                val week = if (weeklyTarget == null) {
                    stringResource(R.string.home_week_hours, weekHours)
                } else {
                    stringResource(R.string.home_week_hours_of_target, weekHours, weeklyTarget)
                }
                Text(week, style = QuestType.small, color = colors.textFaint, modifier = Modifier.testTag("week-hours"))
            }
        }
    }
}

/** A list section title in caps ("QUEST WORK", "OTHER"), with a rule and optional actions. */
@Composable
fun SectionHeader(title: String, modifier: Modifier = Modifier, actions: @Composable () -> Unit = {}) {
    val colors = QuestTheme.colors
    Row(
        modifier.fillMaxWidth().heightIn(min = 36.dp),
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.large),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title.uppercase(), style = QuestType.caps, color = colors.textMuted, modifier = Modifier.semantics { heading() })
        Box(Modifier.weight(1f).height(QuestSpacing.hairline).background(colors.border))
        actions()
    }
}
