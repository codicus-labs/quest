package app.quest.android.ui.plan

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.awaitLongPressOrCancellation
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedback
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import app.quest.android.ui.components.TaskCard
import app.quest.android.ui.components.TaskCardActions
import app.quest.android.ui.theme.QuestSpacing

/** A card under the finger when a drag starts: what it is, and where it sits on the surface. */
class Grab(val card: PlanRow.Card, val topLeft: Offset, val size: IntSize)

/**
 * A plan layout the drag works on (the compact list, the column board). Points are in the
 * surface's own pixels, the same as the drag gesture's.
 */
interface DragSurface {
    fun cardAt(point: Offset): Grab?

    fun targetAt(point: Offset, dragged: DraggedCard): DropTarget?

    /** All rows of [day], to tell a drop that changes nothing. */
    fun rowsOf(day: String): List<PlanRow>

    /** Scrolls when [point] rests at an edge, for [seconds] of frame time; true when it moved. */
    suspend fun autoScroll(point: Offset, seconds: Float): Boolean
}

/**
 * The drag in progress, if any. Only the floating card reads the pointer (in its graphics layer),
 * so moving the finger recomposes nothing; rows read just their own part through [lineFor] and [isDragged].
 */
@Stable
class PlanDragState {
    var dragged by mutableStateOf<DraggedCard?>(null)
        private set
    var target by mutableStateOf<DropTarget?>(null)
    var pointer by mutableStateOf(Offset.Zero)
    var grab by mutableStateOf(Offset.Zero)
        private set
    var size by mutableStateOf(IntSize.Zero)
        private set

    /** Whether the finger moved since the long press; a press without moving opens "Move to…". */
    var moved = false
        private set

    val active get() = dragged != null

    fun start(grab: Grab, at: Offset) {
        dragged = DraggedCard(grab.card.task, grab.card.day, grab.card.section)
        size = grab.size
        this.grab = at - grab.topLeft
        pointer = at
        moved = false
        target = null
    }

    fun move(by: Offset, slop: Float, travelled: Float) {
        pointer += by
        if (travelled > slop) moved = true
    }

    fun clear() {
        dragged = null
        target = null
    }

    @Composable
    fun lineFor(key: String): DropLine? {
        val line by remember(key) { derivedStateOf { target?.line?.takeIf { it.key == key } } }
        return line
    }

    @Composable
    fun isDragged(taskId: String): Boolean {
        val dragging by remember(taskId) { derivedStateOf { dragged?.task?.id == taskId } }
        return dragging
    }
}

/** What a drag ends in: a move ([DropTarget] for [DraggedCard]) or, without moving, "Move to…". */
class DragCallbacks(
    val drop: (DraggedCard, DropTarget) -> Unit,
    val menu: (DraggedCard) -> Unit,
)

/**
 * Long-press a card on [surface] to lift it; dragging shows where it lands, releasing drops it.
 * On the container rather than the cards, so a card scrolling away keeps the gesture alive. Once
 * lifted, moves are taken in the initial pass, before the list's scrolling and the card's swipe see them.
 */
fun Modifier.planDrag(state: PlanDragState, surface: () -> DragSurface, callbacks: () -> DragCallbacks, haptics: HapticFeedback, slop: Float): Modifier =
    pointerInput(state) {
        awaitEachGesture {
            val down = awaitFirstDown(requireUnconsumed = false)
            val press = awaitLongPressOrCancellation(down.id) ?: return@awaitEachGesture
            val grab = surface().cardAt(press.position) ?: return@awaitEachGesture
            state.start(grab, press.position)
            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
            var travelled = 0f
            while (true) {
                val event = awaitPointerEvent(PointerEventPass.Initial)
                val change = event.changes.firstOrNull { it.id == press.id } ?: break
                if (!change.pressed) {
                    // Consumed by someone else (back gesture, shade): a cancel, not a drop.
                    if (change.isConsumed) break
                    change.consume()
                    finish(state, surface(), callbacks(), haptics)
                    return@awaitEachGesture
                }
                val by = change.positionChange()
                change.consume()
                travelled += by.getDistance()
                state.move(by, slop, travelled)
                if (!state.moved) continue
                val dragged = state.dragged ?: break
                val next = surface().targetAt(state.pointer, dragged)
                if (next?.day != state.target?.day) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
                state.target = next
            }
            state.clear()
        }
    }

/** The finger came up: drop where the line shows, or (without moving) open "Move to…". */
private fun finish(state: PlanDragState, surface: DragSurface, callbacks: DragCallbacks, haptics: HapticFeedback) {
    val dragged = state.dragged
    val target = state.target
    val moved = state.moved
    state.clear()
    when {
        dragged == null -> Unit
        !moved -> callbacks.menu(dragged)
        target == null -> Unit
        !isNoMove(surface.rowsOf(target.day), dragged, target) -> {
            haptics.performHapticFeedback(HapticFeedbackType.GestureEnd)
            callbacks.drop(dragged, target)
        }
    }
}

/** The surface's drag: gesture modifier plus the auto-scroll loop at the edges. */
@Composable
fun rememberPlanDrag(state: PlanDragState, surface: DragSurface, callbacks: DragCallbacks): Modifier {
    val currentSurface by rememberUpdatedState(surface)
    val currentCallbacks by rememberUpdatedState(callbacks)
    val haptics = LocalHapticFeedback.current
    val slop = LocalViewConfiguration.current.touchSlop
    LaunchedEffect(state.active) {
        if (!state.active) return@LaunchedEffect
        var last = withFrameNanos { it }
        while (state.active) {
            val now = withFrameNanos { it }
            val seconds = (now - last) / 1_000_000_000f
            last = now
            val dragged = state.dragged ?: break
            if (state.moved && currentSurface.autoScroll(state.pointer, seconds)) {
                state.target = currentSurface.targetAt(state.pointer, dragged)
            }
        }
    }
    return remember(state, haptics, slop) { Modifier.planDrag(state, { currentSurface }, { currentCallbacks }, haptics, slop) }
}

/** The lifted card following the finger, drawn above the surface. */
@Composable
fun DraggedCardOverlay(state: PlanDragState) {
    val dragged = state.dragged ?: return
    val width = with(LocalDensity.current) { state.size.width.toDp() }
    Box(
        Modifier
            .graphicsLayer {
                translationX = state.pointer.x - state.grab.x
                translationY = state.pointer.y - state.grab.y
                scaleX = 1.03f
                scaleY = 1.03f
                shadowElevation = 12.dp.toPx()
                shape = RoundedCornerShape(QuestSpacing.cardRadius)
                clip = false
            }
            .width(width)
            .testTag("plan-dragged"),
    ) { TaskCard(dragged.task, NoCardActions) }
}

private val NoCardActions = TaskCardActions({}, {}, {})
