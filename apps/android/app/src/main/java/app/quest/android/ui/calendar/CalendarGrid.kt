package app.quest.android.ui.calendar

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.first
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerInputChange
import androidx.compose.ui.input.pointer.changedToUp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.Block
import app.quest.android.ui.components.HourLabels
import app.quest.android.ui.components.NowLine
import app.quest.android.ui.components.blockBounds
import app.quest.android.ui.components.drawHourLines
import app.quest.android.ui.components.minuteY
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import kotlin.math.abs
import uniffi.quest_mobile.CalendarDay
import uniffi.quest_mobile.GridBlock
import uniffi.quest_mobile.GridBlockKind
import uniffi.quest_mobile.GridDrag
import uniffi.quest_mobile.GridDragKind
import uniffi.quest_mobile.GridPlacement

private const val DAY_MINUTES = 24f * 60f

/**
 * A handle's touch area: up to a full touch target centred on the block's edge (the pill itself stays
 * small). Inwards it reaches at most a third of the block, so a short block's middle stays free for
 * long-press; outwards at most [HandleOutsideReach], so scrolling beside the block stays free. Over
 * another block only that block's own handles count, so the zone never takes a neighbour's long-press.
 */
private val HandleTouchWidth = 48.dp
private val HandleTouchHeight = 48.dp
private val HandleOutsideReach = 12.dp

/** What the grid's gestures do; the core decides where things land. */
class CalendarGridActions(
    /** The ghost for a drag with the finger over day/minute; null: it can't land there. */
    val preview: (GridDrag, day: String, minute: Float) -> GridPlacement?,
    val drop: (GridDrag, day: String, minute: Float) -> Unit,
    /** A tap on a block: a session opens the entry editor, a planned block its task. */
    val tapBlock: (GridBlock) -> Unit,
    /** Auto-scroll speed in dp/s for the finger at pos within start..end (dp); 0 outside the edge zones. */
    val edgeSpeed: (pos: Float, start: Float, end: Float) -> Float = { _, _, _ -> 0f },
)

/** The drag in progress, if any, and where it would land. Lives in the screen, not the ViewModel. */
@Stable
class CalendarDragState {
    var drag by mutableStateOf<GridDrag?>(null)
    var placement by mutableStateOf<GridPlacement?>(null)

    /** The finger in the grid's content pixels while dragging, and the day under it. */
    var pointer by mutableStateOf(Offset.Zero)
    var overDay by mutableStateOf("")

    fun clear() {
        drag = null
        placement = null
    }
}

/**
 * The calendar's days side by side over 24 hours: hour labels, each day's blocks and now-line, and
 * the drag ghost. Gestures (Kotlin only maps the finger to a day and an unsnapped minute):
 * - tap a block: [CalendarGridActions.tapBlock];
 * - long-press a movable block and drag: move it (to another visible day too);
 * - drag a block's top or bottom handle right away: change its start or end;
 * - long-press empty space and drag: a span to log (the sheet opens on release).
 * Before a long-press nothing is consumed, so the grid scrolls and the page swipes as usual.
 */
@Composable
fun CalendarGrid(
    days: List<CalendarDay>,
    scroll: ScrollState,
    dragState: CalendarDragState,
    actions: CalendarGridActions,
    modifier: Modifier = Modifier,
    hourHeight: Dp = 52.dp,
) {
    val colors = QuestTheme.colors
    EdgeAutoScroll(days, scroll, dragState, actions, hourHeight)
    Column(modifier.background(colors.grid).verticalScroll(scroll).testTag("calendar-grid")) {
        Row(Modifier.fillMaxWidth().height(hourHeight * 24)) {
            days.firstOrNull()?.let { HourLabels(it.grid, hourHeight) }
            BoxWithConstraints(
                Modifier
                    .weight(1f)
                    .fillMaxHeight()
                    .drawBehind {
                        days.firstOrNull()?.let { drawHourLines(it.grid, hourHeight, colors.gridLine) }
                        val column = size.width / days.size.coerceAtLeast(1)
                        for (i in 1 until days.size) {
                            drawLine(colors.gridLine, Offset(column * i, 0f), Offset(column * i, size.height), strokeWidth = 1.dp.toPx())
                        }
                    }
                    .gridGestures(days, hourHeight, dragState, actions),
            ) {
                val columnWidth = maxWidth / days.size.coerceAtLeast(1)
                days.forEachIndexed { index, day ->
                    Box(Modifier.offset(x = columnWidth * index).width(columnWidth).fillMaxHeight().testTag("calendar-day:${day.grid.day}")) {
                        day.grid.blocks.forEach { block -> CalendarBlock(block, day.grid.day, columnWidth, hourHeight, dragState, actions) }
                        day.grid.nowMinute?.let { NowLine(it, hourHeight) }
                    }
                }
                dragState.placement?.let { placement ->
                    val index = days.indexOfFirst { it.grid.day == placement.day }
                    if (index >= 0) Ghost(placement, columnWidth, hourHeight, Modifier.offset(x = columnWidth * index))
                }
            }
        }
    }
}

/**
 * While a drag rests in the top or bottom edge zone, scrolls the grid at the core's edge speed
 * (same zone as the plan). The finger stays put on screen, so it moves through the content: the
 * pointer and the ghost follow the scroll.
 */
@Composable
private fun EdgeAutoScroll(days: List<CalendarDay>, scroll: ScrollState, dragState: CalendarDragState, actions: CalendarGridActions, hourHeight: Dp) {
    val density = LocalDensity.current
    val currentDays by rememberUpdatedState(days)
    val currentActions by rememberUpdatedState(actions)
    val dragging = dragState.drag != null
    LaunchedEffect(dragging) {
        if (!dragging) return@LaunchedEffect
        val dp = density.density
        fun speed() = currentActions.edgeSpeed((dragState.pointer.y - scroll.value) / dp, 0f, scroll.viewportSize / dp)
        while (dragState.drag != null) {
            // Outside the edge zones no frames are asked for: wait until the finger enters one.
            snapshotFlow { dragState.drag == null || speed() != 0f }.first { it }
            var last = withFrameNanos { it }
            while (true) {
                val now = withFrameNanos { it }
                val seconds = (now - last) / 1_000_000_000f
                last = now
                val drag = dragState.drag ?: break
                val speed = speed()
                if (speed == 0f) break
                val moved = scroll.scrollBy(speed * seconds * dp)
                // At the top or bottom: nothing more to scroll until the finger moves.
                if (moved == 0f) break
                dragState.pointer = dragState.pointer.copy(y = dragState.pointer.y + moved)
                val day = dragState.overDay.takeIf { d -> currentDays.any { it.grid.day == d } } ?: continue
                // Null where the core can't land the drag: the ghost goes, as in the gesture loop.
                dragState.placement = currentActions.preview(drag, day, minuteAt(dragState.pointer.y, hourHeight.value * dp))
            }
            if (dragState.drag != null && speed() != 0f) snapshotFlow { dragState.pointer }.drop(1).first()
        }
    }
}

/** How far one accessibility action moves a block or its end, as a drag of that many minutes would; the core places it. */
private const val A11Y_STEP_MINUTES = 15f

@Composable
private fun CalendarBlock(block: GridBlock, day: String, columnWidth: Dp, hourHeight: Dp, dragState: CalendarDragState, actions: CalendarGridActions) {
    val dragged = dragState.drag?.blockId == block.id
    val tapLabel = stringResource(if (block.kind == GridBlockKind.PLANNED) R.string.task_open else R.string.calendar_edit_entry)
    val earlier = stringResource(R.string.calendar_move_earlier)
    val later = stringResource(R.string.calendar_move_later)
    val endEarlier = stringResource(R.string.calendar_end_earlier)
    val endLater = stringResource(R.string.calendar_end_later)
    // The drag gestures as actions for TalkBack and switch access: a drag by one step, dropped at once.
    val stepActions = buildList {
        fun step(label: String, kind: GridDragKind, fromMinute: UInt, by: Float) = add(
            CustomAccessibilityAction(label) {
                val from = fromMinute.toFloat()
                actions.drop(GridDrag(kind, block.id, day, from), day, from + by)
                true
            },
        )
        if (block.canMove) {
            step(earlier, GridDragKind.MOVE, block.startMinute, -A11Y_STEP_MINUTES)
            step(later, GridDragKind.MOVE, block.startMinute, A11Y_STEP_MINUTES)
        }
        if (block.endHandle) {
            step(endEarlier, GridDragKind.END_EDGE, block.endMinute, -A11Y_STEP_MINUTES)
            step(endLater, GridDragKind.END_EDGE, block.endMinute, A11Y_STEP_MINUTES)
        }
    }
    Box {
        Block(
            block,
            columnWidth,
            hourHeight,
            Modifier
                .alpha(if (dragged) 0.35f else 1f)
                .semantics {
                    role = Role.Button
                    onClick(tapLabel) {
                        actions.tapBlock(block)
                        true
                    }
                    customActions = stepActions
                },
        )
        Handles(block, columnWidth, hourHeight)
    }
}

/** Small pills on the edges that drag (top: start, bottom: end). */
@Composable
private fun Handles(block: GridBlock, columnWidth: Dp, hourHeight: Dp) {
    if (!block.startHandle && !block.endHandle) return
    val bounds = blockBounds(block, columnWidth, hourHeight)
    val color = block.colorHex?.let(::questColor) ?: QuestTheme.colors.textFaint
    val pill = Modifier.width(16.dp).height(4.dp).background(color, RoundedCornerShape(2.dp))
    val x = bounds.x + bounds.width / 2 - 8.dp
    if (block.startHandle) Box(Modifier.offset(x = x, y = bounds.y - 2.dp).then(pill).testTag("grid-handle-start:${block.id}"))
    if (block.endHandle) Box(Modifier.offset(x = x, y = bounds.y + bounds.height - 2.dp).then(pill).testTag("grid-handle-end:${block.id}"))
}

/** Where a drop would land: an accent outline with its times. */
@Composable
private fun Ghost(placement: GridPlacement, columnWidth: Dp, hourHeight: Dp, modifier: Modifier) {
    val accent = QuestTheme.colors.accent
    val shape = RoundedCornerShape(4.dp)
    val top = minuteY(placement.startMinute.toFloat(), hourHeight)
    val height = minuteY((placement.endMinute - placement.startMinute).toFloat(), hourHeight)
    Box(
        modifier
            .offset(y = top)
            .padding(horizontal = 2.dp)
            .width(columnWidth - 4.dp)
            .height(height)
            .background(accent.copy(alpha = 0.18f), shape)
            .border(1.5.dp, accent, shape)
            .testTag("calendar-ghost"),
    ) {
        Text(
            placement.rangeLabel,
            style = QuestType.smallMedium,
            color = QuestTheme.colors.text,
            maxLines = 1,
            modifier = Modifier.padding(horizontal = QuestSpacing.small, vertical = 2.dp),
        )
    }
}

/** What the finger went down on. */
private sealed interface GridHit {
    data class Handle(val block: GridBlock, val kind: GridDragKind) : GridHit

    data class OnBlock(val block: GridBlock) : GridHit

    data object Empty : GridHit
}

/** The day columns' geometry in px, from the grid's width. */
private class GridGeometry(val width: Float, val dayCount: Int, val hourHeightPx: Float) {
    val columnWidth get() = width / dayCount.coerceAtLeast(1)

    fun dayIndex(x: Float) = (x / columnWidth).toInt().coerceIn(0, dayCount - 1)

    fun minute(y: Float) = minuteAt(y, hourHeightPx)
}

/** The minute of the day at [y] px from the grid's top. */
private fun minuteAt(y: Float, hourHeightPx: Float) = (y / hourHeightPx * 60f).coerceIn(0f, DAY_MINUTES)

private fun Modifier.gridGestures(days: List<CalendarDay>, hourHeight: Dp, dragState: CalendarDragState, actions: CalendarGridActions): Modifier =
    composed {
        val haptics = LocalHapticFeedback.current
        val density = LocalDensity.current
        val currentDays by rememberUpdatedState(days)
        val currentActions by rememberUpdatedState(actions)
        pointerInput(hourHeight) {
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                val geometry = GridGeometry(size.width.toFloat(), currentDays.size, with(density) { hourHeight.toPx() })
                val dayIndex = geometry.dayIndex(down.position.x)
                val day = currentDays.getOrNull(dayIndex) ?: return@awaitEachGesture
                val local = Offset(down.position.x - geometry.columnWidth * dayIndex, down.position.y)
                val hit = hitTest(day, local, geometry.columnWidth, hourHeight, density)
                val slop = viewConfiguration.touchSlop
                // Until the long-press: a tap, a handle drag, or someone else's gesture (scroll, swipe).
                var longPressed = false
                val early = withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
                    while (true) {
                        val event = awaitPointerEvent()
                        val change = event.changes.firstOrNull { it.id == down.id } ?: return@withTimeoutOrNull EarlyEnd.Cancelled
                        if (change.changedToUp()) return@withTimeoutOrNull EarlyEnd.Tapped
                        if (change.isConsumed) return@withTimeoutOrNull EarlyEnd.Cancelled
                        val moved = change.position - down.position
                        if (moved.getDistance() > slop) {
                            val vertical = abs(moved.y) > abs(moved.x)
                            if (hit !is GridHit.Handle || !vertical) return@withTimeoutOrNull EarlyEnd.Cancelled
                            // Taken here, so the scroll behind it doesn't start with the same move.
                            change.consume()
                            return@withTimeoutOrNull EarlyEnd.HandleDrag
                        }
                    }
                    @Suppress("UNREACHABLE_CODE")
                    EarlyEnd.Cancelled
                }
                val drag: GridDrag = when (early) {
                    EarlyEnd.Cancelled -> return@awaitEachGesture
                    EarlyEnd.Tapped -> {
                        when (hit) {
                            is GridHit.Handle -> currentActions.tapBlock(hit.block)
                            is GridHit.OnBlock -> currentActions.tapBlock(hit.block)
                            GridHit.Empty -> Unit
                        }
                        return@awaitEachGesture
                    }
                    EarlyEnd.HandleDrag -> {
                        val handle = hit as GridHit.Handle
                        GridDrag(handle.kind, handle.block.id, day.grid.day, geometry.minute(down.position.y))
                    }
                    null -> {
                        longPressed = true
                        val block = when (hit) {
                            is GridHit.Handle -> hit.block
                            is GridHit.OnBlock -> hit.block
                            GridHit.Empty -> null
                        }
                        when {
                            block == null -> GridDrag(GridDragKind.NEW_SPAN, null, day.grid.day, geometry.minute(down.position.y))
                            block.canMove -> GridDrag(GridDragKind.MOVE, block.id, day.grid.day, geometry.minute(down.position.y))
                            // A block that can't move (running, cut at midnight): a long-press is just a tap.
                            else -> {
                                waitForUp(down.id)?.let { currentActions.tapBlock(block) }
                                return@awaitEachGesture
                            }
                        }
                    }
                }
                if (longPressed) haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                dragState.drag = drag
                dragState.pointer = down.position
                dragState.overDay = day.grid.day
                dragState.placement = currentActions.preview(drag, day.grid.day, geometry.minute(down.position.y))
                var last = down.position
                var released = false
                val landed = try {
                    while (true) {
                        val event = awaitPointerEvent()
                        val change = event.changes.firstOrNull { it.id == down.id } ?: break
                        if (!change.pressed) {
                            // A cancelled gesture (system back, a shade pulled down) arrives already consumed.
                            released = !change.isConsumed
                            // After an auto-scroll the same screen point is a different content point.
                            last = change.position
                            change.consume()
                            break
                        }
                        change.consume()
                        if (change.position == last) continue
                        last = change.position
                        dragState.pointer = last
                        // The page may have fewer days by now (window resized mid-drag).
                        val target = currentDays.getOrNull(geometry.dayIndex(last.x))?.grid?.day ?: continue
                        dragState.overDay = target
                        val next = currentActions.preview(drag, target, geometry.minute(last.y))
                        if (next != null && next.day != dragState.placement?.day) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
                        dragState.placement = next
                    }
                    dragState.placement
                } finally {
                    // Also when the page leaves composition mid-drag: no ghost left behind.
                    dragState.clear()
                }
                val target = currentDays.getOrNull(geometry.dayIndex(last.x))?.grid?.day
                if (released && landed != null && target != null) currentActions.drop(drag, target, geometry.minute(last.y))
            }
        }
    }

private enum class EarlyEnd { Tapped, Cancelled, HandleDrag }

/** Waits for [id] to lift; null when the gesture was cancelled instead. */
private suspend fun androidx.compose.ui.input.pointer.AwaitPointerEventScope.waitForUp(id: androidx.compose.ui.input.pointer.PointerId): PointerInputChange? {
    while (true) {
        val change = awaitPointerEvent().changes.firstOrNull { it.id == id } ?: return null
        if (!change.pressed) return change.takeIf { !it.isConsumed }
    }
}

/** The handle or block under [local] (px inside the day's column), else empty space. Handles win near edges. */
private fun hitTest(day: CalendarDay, local: Offset, columnWidthPx: Float, hourHeight: Dp, density: Density): GridHit = with(density) {
    val columnWidth = columnWidthPx.toDp()
    val x = local.x.toDp()
    val y = local.y.toDp()
    val onBlock = day.grid.blocks.lastOrNull { block ->
        val b = blockBounds(block, columnWidth, hourHeight)
        x >= b.x && x <= b.x + b.width && y >= b.y && y <= b.y + b.height
    }
    var best: GridHit.Handle? = null
    var bestDistance = Float.MAX_VALUE
    for (block in day.grid.blocks) {
        if (onBlock != null && block.id != onBlock.id) continue
        val b = blockBounds(block, columnWidth, hourHeight)
        val centre = b.x + b.width / 2
        val handleHalf = minOf(HandleTouchWidth, b.width) / 2
        val withinHandleX = x >= centre - handleHalf && x <= centre + handleHalf
        if (withinHandleX) {
            val insideReach = minOf(HandleTouchHeight / 2, b.height / 3)
            val outsideReach = minOf(HandleTouchHeight / 2, HandleOutsideReach)
            // How far the touch lies inside the block from each edge (negative: outside it).
            val edges = listOfNotNull(
                (y - b.y to GridDragKind.START_EDGE).takeIf { block.startHandle },
                (b.y + b.height - y to GridDragKind.END_EDGE).takeIf { block.endHandle },
            )
            for ((inward, kind) in edges) {
                val distance = abs(inward.value)
                if (inward >= -outsideReach && inward <= insideReach && distance < bestDistance) {
                    best = GridHit.Handle(block, kind)
                    bestDistance = distance
                }
            }
        }
    }
    best ?: onBlock?.let { GridHit.OnBlock(it) } ?: GridHit.Empty
}
