package app.quest.android.ui.tracked

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.questColor
import app.quest.android.ui.lists.ListMaxWidth
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.TrackedBlock
import uniffi.quest_mobile.TrackedData
import uniffi.quest_mobile.WeekSummary

class TrackedActions(
    val openEntry: (String) -> Unit,
    val openReview: () -> Unit,
    val logWork: () -> Unit,
)

/** How tall one hour of the timeline is. */
private val HourHeight = 44.dp

/** Room on the left for "09". */
private val HourLabelWidth = 32.dp

/** The Tracked tab: the review's entry, today's timeline (a block opens its session), then this week. */
@Composable
fun TrackedScreen(state: TrackedUiState, actions: TrackedActions, modifier: Modifier = Modifier) {
    when (state) {
        TrackedUiState.Loading -> Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is TrackedUiState.Failed -> Box(modifier.fillMaxSize().padding(QuestSpacing.screen), contentAlignment = Alignment.Center) {
            Text(state.message, style = QuestType.body, color = QuestTheme.colors.textMuted)
        }
        is TrackedUiState.Ready -> Column(
            modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .wrapContentWidth(Alignment.CenterHorizontally)
                .widthIn(max = ListMaxWidth)
                .padding(horizontal = QuestSpacing.screen)
                .padding(bottom = 88.dp)
                .testTag("tracked"),
            verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
        ) {
            Header(state.tracked, actions.logWork)
            ReviewLink(actions.openReview)
            Timeline(state.tracked, actions.openEntry)
            Week(state.tracked.week)
        }
    }
}

@Composable
private fun Header(tracked: TrackedData, logWork: () -> Unit) {
    val colors = QuestTheme.colors
    Row(Modifier.fillMaxWidth().padding(top = QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.nav_tracked), style = QuestType.heading, color = colors.text, modifier = Modifier.semantics { heading() })
            Text(stringResource(R.string.tracked_today, tracked.total), style = QuestType.small, color = colors.textMuted)
        }
        TextButton(onClick = logWork, modifier = Modifier.testTag("tracked-log-work")) { Text(stringResource(R.string.log_work)) }
    }
}

@Composable
private fun ReviewLink(open: () -> Unit) {
    val colors = QuestTheme.colors
    val label = stringResource(R.string.tracked_review)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = QuestSpacing.touchTarget)
            .clip(RoundedCornerShape(QuestSpacing.cardRadius))
            .background(colors.card)
            .testTag("tracked-review")
            .clickable(onClick = open)
            .clearAndSetSemantics {
                contentDescription = label
                role = Role.Button
            }
            .padding(horizontal = QuestSpacing.medium),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, style = QuestType.bodyMedium, color = colors.text, modifier = Modifier.weight(1f))
        Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null, tint = colors.textMuted)
    }
}

/** Today from [TrackedData.firstHour] to [TrackedData.lastHour]: hour lines, the sessions, and now. */
@Composable
private fun Timeline(tracked: TrackedData, open: (String) -> Unit) {
    val colors = QuestTheme.colors
    val first = tracked.firstHour.toInt()
    val hours = (tracked.lastHour.toInt() - first).coerceAtLeast(1)
    fun top(minute: Int): Dp = HourHeight * ((minute - first * 60) / 60f)
    Box(Modifier.fillMaxWidth().height(HourHeight * hours).testTag("timeline")) {
        for (hour in 0..hours) {
            Row(Modifier.offset(y = HourHeight * hour - 7.dp), verticalAlignment = Alignment.CenterVertically) {
                Text("%02d".format(first + hour), style = QuestType.small, color = colors.textFaint, modifier = Modifier.width(HourLabelWidth))
                Box(Modifier.weight(1f).height(QuestSpacing.hairline).background(colors.gridLine))
            }
        }
        tracked.blocks.forEach { block ->
            val start = block.startMinute.toInt()
            val height = (top(block.endMinute.toInt()) - top(start)).coerceAtLeast(22.dp)
            Block(block, open, Modifier.offset(x = HourLabelWidth, y = top(start)).height(height))
        }
        if (tracked.nowMinute > 0u) {
            Box(Modifier.offset(x = HourLabelWidth - 4.dp, y = top(tracked.nowMinute.toInt()) - 1.dp).fillMaxWidth().height(2.dp).background(colors.accentLine))
        }
    }
}

@Composable
private fun Block(block: TrackedBlock, open: (String) -> Unit, modifier: Modifier) {
    val colors = QuestTheme.colors
    val tint = block.colorHex?.let(::questColor) ?: if (block.organizing) colors.accentFill else colors.textFaint
    val shape = RoundedCornerShape(6.dp)
    Row(
        modifier
            .fillMaxWidth()
            .padding(end = HourLabelWidth, top = 1.dp, bottom = 1.dp)
            .clip(shape)
            .background(tint.copy(alpha = 0.22f))
            .testTag("block:${block.title}")
            .clickable { open(block.entryId) }
            .semantics(mergeDescendants = true) { role = Role.Button },
    ) {
        Box(Modifier.width(3.dp).fillMaxHeight().background(tint))
        Row(Modifier.padding(horizontal = QuestSpacing.small, vertical = 2.dp), horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
            Text(block.title, style = QuestType.smallMedium, color = colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
            Text(block.range, style = QuestType.small, color = colors.textMuted, maxLines = 1)
        }
    }
}

@Composable
private fun Week(week: WeekSummary) {
    val colors = QuestTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium), modifier = Modifier.testTag("week")) {
        Row(verticalAlignment = Alignment.Bottom) {
            Text(stringResource(R.string.tracked_this_week), style = QuestType.stat, color = colors.text, modifier = Modifier.weight(1f).semantics { heading() })
            Text(week.total, style = QuestType.monoTime, color = colors.text)
        }
        Bar(week.progress, colors.accentFill)
        week.quests.forEach { line -> WeekLine(line.title, line.hours, questColor(line.colorHex), line.fraction) }
        WeekLine(stringResource(R.string.organizing), week.organizing, colors.accentFill, null)
        week.noQuest?.let { WeekLine(stringResource(R.string.no_quest), it, colors.textFaint, null) }
        if (week.postponed > 0u) {
            Text(
                pluralStringResource(R.plurals.tracked_postponed, week.postponed.toInt(), week.postponed.toInt()),
                style = QuestType.small,
                color = colors.textMuted,
            )
        }
    }
}

@Composable
private fun WeekLine(title: String, hours: String, color: Color, fraction: Float?) {
    val colors = QuestTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
            Dot(color, 8.dp)
            Text(title, style = QuestType.body, color = colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            Text(hours, style = QuestType.monoTime, color = colors.textMuted)
        }
        fraction?.let { Bar(it, color, Modifier.padding(start = 16.dp)) }
    }
}

@Composable
private fun Bar(fraction: Float, color: Color, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(50)
    Box(modifier.fillMaxWidth().height(6.dp).clip(shape).background(QuestTheme.colors.barTrack)) {
        Box(Modifier.fillMaxWidth(fraction.coerceIn(0f, 1f)).height(6.dp).background(color, shape))
    }
}
