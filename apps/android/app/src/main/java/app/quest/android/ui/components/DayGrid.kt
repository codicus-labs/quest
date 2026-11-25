package app.quest.android.ui.components

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.DayGrid
import uniffi.quest_mobile.GridBlock
import uniffi.quest_mobile.GridBlockKind

private const val MINUTES_PER_DAY = 24 * 60
internal val HourLabelWidth = 56.dp
internal val MinBlockHeight = 18.dp

/** Where the grid scrolls to first: two hours before now (so the morning's sessions show), else 07:00 (desktop Home). */
private const val DEFAULT_FIRST_MINUTE = 7 * 60
private const val LEAD_MINUTES = 120

/**
 * One day as a read-only time grid (24 h): hour lines, tracked sessions (filled and hatched in
 * their quest colour), the running timer (dashed), planned blocks (outlined) and the now-line.
 * Positions come from the core in minutes after midnight; this only scales them to [hourHeight].
 * Phase 4 builds the Calendar on it (other days, gestures).
 */
@Composable
fun DayGrid(
    grid: DayGrid,
    modifier: Modifier = Modifier,
    hourHeight: Dp = 52.dp,
    scroll: ScrollState = rememberScrollState(),
) {
    val colors = QuestTheme.colors
    val density = LocalDensity.current
    val firstMinute = (grid.nowMinute?.toInt()?.minus(LEAD_MINUTES) ?: DEFAULT_FIRST_MINUTE).coerceAtLeast(0)
    LaunchedEffect(grid.day) {
        scroll.scrollTo(with(density) { (hourHeight * firstMinute / 60f).roundToPx() })
    }
    Column(
        modifier
            .background(colors.grid)
            .verticalScroll(scroll)
            .semantics { contentDescription = grid.dayLabel }
            .testTag("day-grid"),
    ) {
        Row(Modifier.fillMaxWidth().height(hourHeight * 24)) {
            HourLabels(grid, hourHeight)
            BoxWithConstraints(Modifier.weight(1f).fillMaxHeight().drawBehind { drawHourLines(grid, hourHeight, colors.gridLine) }) {
                val width = maxWidth
                grid.blocks.forEach { block -> Block(block, width, hourHeight) }
                grid.nowMinute?.let { NowLine(it, hourHeight) }
            }
        }
    }
}

internal fun minuteY(minute: Float, hourHeight: Dp): Dp = hourHeight * (minute / 60f)

@Composable
internal fun HourLabels(grid: DayGrid, hourHeight: Dp) {
    // The column has a fixed width; "09:00" must stay on one line.
    CappedFontScale(HOUR_LABEL_MAX_FONT_SCALE) {
        Box(Modifier.width(HourLabelWidth).fillMaxHeight()) {
            grid.hours.forEach { mark ->
                Text(
                    mark.label,
                    style = QuestType.small,
                    color = QuestTheme.colors.textFaint,
                    textAlign = TextAlign.End,
                    maxLines = 1,
                    modifier = Modifier.width(HourLabelWidth - QuestSpacing.medium).offset(y = minuteY(mark.minute.toFloat(), hourHeight) - 8.dp),
                )
            }
        }
    }
}

private const val HOUR_LABEL_MAX_FONT_SCALE = 1.2f

internal fun androidx.compose.ui.graphics.drawscope.DrawScope.drawHourLines(grid: DayGrid, hourHeight: Dp, color: Color) {
    grid.hours.forEach { mark ->
        val y = minuteY(mark.minute.toFloat(), hourHeight).toPx()
        drawLine(color, Offset(0f, y), Offset(size.width, y), strokeWidth = 1.dp.toPx())
    }
}

/** Where [block] sits in a day column [columnWidth] wide (its lane, its minutes); also what drags hit. */
internal class BlockBounds(val x: Dp, val y: Dp, val width: Dp, val height: Dp)

internal fun blockBounds(block: GridBlock, columnWidth: Dp, hourHeight: Dp): BlockBounds {
    val laneWidth = columnWidth / block.laneCount.coerceAtLeast(1u).toInt()
    return BlockBounds(
        x = laneWidth * block.lane.toInt() + 2.dp,
        y = minuteY(block.startMinute.toFloat(), hourHeight) + 1.dp,
        width = laneWidth - 4.dp,
        height = (minuteY((block.endMinute - block.startMinute).toFloat(), hourHeight) - 2.dp).coerceAtLeast(MinBlockHeight),
    )
}

/** One block; [modifier] adds what the calendar needs (its tap for TalkBack, fading while dragged). */
@Composable
internal fun Block(block: GridBlock, columnWidth: Dp, hourHeight: Dp, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val color = block.colorHex?.let(::questColor) ?: colors.textFaint
    val bounds = blockBounds(block, columnWidth, hourHeight)
    val height = bounds.height
    val shape = RoundedCornerShape(4.dp)
    Row(
        Modifier
            .offset(x = bounds.x, y = bounds.y)
            .width(bounds.width)
            .height(height)
            .then(modifier)
            .clip(shape)
            .blockStyle(block.kind, color)
            .semantics { contentDescription = block.a11yLabel }
            .testTag("grid-block:${block.id}"),
    ) {
        if (block.kind == GridBlockKind.SESSION) Box(Modifier.width(3.dp).fillMaxHeight().background(color))
        BlockText(block, color, height)
    }
}

/**
 * Title, and the time when both lines fit at the current text size; in a block shorter than padded title the
 * padding goes, so the title (one line, ellipsized) is cut as little as possible.
 */
@Composable
private fun BlockText(block: GridBlock, color: Color, height: Dp) {
    val line = with(LocalDensity.current) { QuestType.small.lineHeight.toDp() }
    val padded = line + 8.dp
    val showRange = height >= padded + line + 2.dp
    val verticalPadding = if (height >= padded) 4.dp else 0.dp
    val colors = QuestTheme.colors
    val titleColor = when {
        block.kind == GridBlockKind.RUNNING -> color
        block.kind == GridBlockKind.PLANNED -> if (block.done) colors.textFaint else colors.textMuted
        else -> colors.text
    }
    Column(Modifier.padding(horizontal = QuestSpacing.small, vertical = verticalPadding), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        // Sessions show the time above the title; planned blocks below it (desktop).
        if (showRange && block.kind == GridBlockKind.SESSION) RangeText(block.rangeLabel, colors.textMuted)
        Text(block.title, style = QuestType.smallMedium, color = titleColor, maxLines = 1, overflow = TextOverflow.Ellipsis)
        if (showRange && block.kind == GridBlockKind.PLANNED) RangeText(block.rangeLabel, colors.textFaint)
    }
}

@Composable
private fun RangeText(range: String, color: Color) = Text(range, style = QuestType.small, color = color, maxLines = 1)

/** Session: filled and hatched; running: dashed outline; planned: outline only (desktop `timegrid.rs`). */
private fun Modifier.blockStyle(kind: GridBlockKind, color: Color): Modifier = when (kind) {
    GridBlockKind.SESSION -> background(color.copy(alpha = 0.20f)).drawBehind { hatch(color.copy(alpha = 0.28f)) }
    GridBlockKind.RUNNING -> background(color.copy(alpha = 0.10f)).drawWithContent {
        drawContent()
        drawRoundRect(
            color.copy(alpha = 0.85f),
            cornerRadius = CornerRadius(4.dp.toPx()),
            style = Stroke(1.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 3.dp.toPx()))),
        )
    }
    GridBlockKind.PLANNED -> border(1.dp, color.copy(alpha = 0.55f), RoundedCornerShape(4.dp))
}

private fun androidx.compose.ui.graphics.drawscope.DrawScope.hatch(color: Color) {
    val pitch = 7.dp.toPx()
    clipRect {
        var x = -size.height
        while (x < size.width) {
            drawLine(color, Offset(x, size.height), Offset(x + size.height, 0f), strokeWidth = 1.dp.toPx())
            x += pitch
        }
    }
}

@Composable
internal fun NowLine(minute: Float, hourHeight: Dp) {
    val accent = QuestTheme.colors.accent
    val y = minuteY(minute.coerceIn(0f, MINUTES_PER_DAY.toFloat()), hourHeight)
    Box(Modifier.fillMaxWidth().offset(y = y - 5.dp).height(10.dp).testTag("now-line")) {
        Box(Modifier.fillMaxWidth().height(2.dp).offset(y = 4.dp).background(accent))
        Box(Modifier.size(10.dp).offset(x = (-5).dp).background(accent, RoundedCornerShape(50)))
    }
}

/** A day grid in Home's card frame (rounded, grid fill). */
@Composable
fun DayGridCard(grid: DayGrid, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(12.dp)
    Box(modifier.clip(shape).border(QuestSpacing.hairline, colors.border, shape)) {
        DayGrid(grid, Modifier.fillMaxSize())
    }
}
