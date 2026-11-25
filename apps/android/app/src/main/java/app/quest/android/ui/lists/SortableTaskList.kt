package app.quest.android.ui.lists

import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.awaitLongPressOrCancellation
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import app.quest.android.R
import app.quest.android.ui.theme.QuestTheme
import uniffi.quest_mobile.ListRow
import uniffi.quest_mobile.PlanTaskList

internal typealias PlaceTask = (String, PlanTaskList, String?) -> Unit

internal sealed interface SortItem {
    val key: String
    val list: PlanTaskList?

    data class Row(override val key: String, override val list: PlanTaskList?, val task: ListRow) : SortItem

    class Decoration(
        override val key: String,
        override val list: PlanTaskList? = null,
        val content: @Composable () -> Unit,
    ) : SortItem
}

internal data class TaskPlacement(val list: PlanTaskList, val before: String?)
private data class LiftedRow(val row: SortItem.Row, val height: Int, val grab: Offset, val width: Int)

private fun placementAt(items: List<SortItem>, index: Int, after: Boolean, taskId: String): TaskPlacement? {
    val item = items.getOrNull(index) ?: return null
    val list = item.list ?: return null
    val before = items.asSequence()
        .drop(index + if (after) 1 else 0)
        .takeWhile { it.list == list }
        .filterIsInstance<SortItem.Row>()
        .firstOrNull { !it.task.done && it.task.id != taskId }
    return TaskPlacement(list, before?.task?.id)
}

private fun List<SortItem>.withGap(lifted: LiftedRow?, target: TaskPlacement?): List<SortItem> {
    if (lifted == null || target == null) return this
    val items = filterNot { it.key == lifted.row.key }.toMutableList()
    val anchor = items.indexOfFirst { it is SortItem.Row && it.list == target.list && it.task.id == target.before }
    val lastOpenRow = items.indexOfLast { it is SortItem.Row && it.list == target.list && !it.task.done }
    val gapIndex = when {
        anchor >= 0 -> anchor
        lastOpenRow >= 0 -> lastOpenRow + 1
        else -> items.indexOfFirst { it.list == target.list }.takeIf { it >= 0 }?.plus(1) ?: return this
    }
    items.add(gapIndex, lifted.row)
    return items
}

@Composable
internal fun SortableTaskList(
    state: LazyListState,
    entries: List<SortItem>,
    actions: RowActions,
    place: PlaceTask,
    modifier: Modifier = Modifier,
    planToday: String? = null,
    content: @Composable BoxScope.(List<SortItem>, @Composable (SortItem, Modifier) -> Unit) -> Unit,
) {
    val density = LocalDensity.current
    val haptics = LocalHapticFeedback.current
    val context = LocalContext.current
    var lifted by remember(state) { mutableStateOf<LiftedRow?>(null) }
    var pointer by remember(state) { mutableStateOf(Offset.Zero) }
    var destination by remember(state) { mutableStateOf<TaskPlacement?>(null) }
    val currentEntries by rememberUpdatedState(entries)
    val currentPlace by rememberUpdatedState(place)
    val currentPlanToday by rememberUpdatedState(planToday)

    fun targetAt(): TaskPlacement? {
        val drag = lifted ?: return null
        val info = state.layoutInfo
        if (pointer.x !in 0f..info.viewportSize.width.toFloat() ||
            pointer.y !in 0f..info.viewportSize.height.toFloat()
        ) return null
        val item = info.visibleItemsInfo.firstOrNull { pointer.y < it.offset + it.size }
            ?: info.visibleItemsInfo.lastOrNull() ?: return null
        if (item.key == drag.row.key) return destination
        val index = currentEntries.indexOfFirst { it.key == item.key }
        return placementAt(currentEntries, index, pointer.y >= item.offset + item.size / 2f, drag.row.task.id)
    }

    fun edgeStrength(): Float {
        if (lifted == null) return 0f
        val viewport = state.layoutInfo.viewportSize
        val edge = with(density) { 72.dp.toPx() }.coerceAtMost(viewport.height / 2f)
        if (edge <= 0 || pointer.x !in 0f..viewport.width.toFloat() || pointer.y !in 0f..viewport.height.toFloat()) return 0f
        return when {
            pointer.y < edge && state.canScrollBackward -> (pointer.y - edge) / edge
            pointer.y > viewport.height - edge && state.canScrollForward -> (pointer.y - (viewport.height - edge)) / edge
            else -> 0f
        }
    }

    val atEdge by remember(state, density) { derivedStateOf { edgeStrength() != 0f } }
    LaunchedEffect(atEdge) {
        if (!atEdge) return@LaunchedEffect
        var last = withFrameNanos { it }
        while (true) {
            val now = withFrameNanos { it }
            val strength = edgeStrength()
            if (strength == 0f) break
            val distance = with(density) { 900.dp.toPx() } * strength * ((now - last) / 1e9f).coerceAtMost(0.05f)
            if (state.scrollBy(distance) != 0f) destination = targetAt()
            last = now
        }
    }

    val gestures = Modifier.pointerInput(state, haptics) {
        awaitEachGesture {
            val down = awaitFirstDown(requireUnconsumed = false)
            val press = awaitLongPressOrCancellation(down.id) ?: return@awaitEachGesture
            val item = state.layoutInfo.visibleItemsInfo.firstOrNull { press.position.y >= it.offset && press.position.y < it.offset + it.size }
                ?: return@awaitEachGesture
            val row = currentEntries.firstOrNull { it.key == item.key } as? SortItem.Row ?: return@awaitEachGesture
            if (row.task.done) return@awaitEachGesture

            pointer = press.position
            lifted = LiftedRow(row, item.size, pointer - Offset(0f, item.offset.toFloat()), size.width)
            val original = placementAt(currentEntries, currentEntries.indexOf(row), after = true, taskId = row.task.id)
            destination = original
            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
            try {
                while (true) {
                    val event = awaitPointerEvent(PointerEventPass.Initial)
                    val change = event.changes.firstOrNull { it.id == press.id } ?: break
                    if (change.isConsumed || event.changes.any { it.id != press.id && it.pressed }) break
                    change.consume()
                    if (!change.pressed) {
                        val target = destination
                        if (target != null && target != original) {
                            val list = currentPlanToday?.takeIf { row.list == null && target.list == PlanTaskList.Today }
                                ?.let { PlanTaskList.Day(it) } ?: target.list
                            currentPlace(row.task.id, list, target.before)
                        }
                        break
                    }
                    if (pointer != change.position) {
                        pointer = change.position
                        destination = targetAt()
                    }
                }
            } finally {
                lifted = null
                destination = null
            }
        }
    }
    val drag = lifted
    val placed = entries.withGap(drag, destination)
    Box(modifier.then(gestures)) {
        content(placed) { item, animation ->
            if (drag?.row?.key == item.key) {
                Box(
                    Modifier.fillMaxWidth().height(with(density) { drag.height.toDp() })
                        .clip(RowShape)
                        .background(QuestTheme.colors.textMuted.copy(alpha = 0.08f))
                        .border(1.dp, QuestTheme.colors.borderStrong, RowShape)
                        .testTag("drop-gap"),
                )
            } else {
                Box(animation.fillMaxWidth()) {
                    when (item) {
                        is SortItem.Row -> TaskRow(item.task, actions, reorderActions(item, entries, place))
                        is SortItem.Decoration -> item.content()
                    }
                }
            }
        }
        if (drag != null) {
            val bg = QuestTheme.colors.card
            Box(
                Modifier.zIndex(1f)
                    .width(with(density) { drag.width.toDp() })
                    .graphicsLayer {
                        translationX = pointer.x - drag.grab.x
                        translationY = pointer.y - drag.grab.y
                        shape = RowShape
                        shadowElevation = 8.dp.toPx()
                        clip = true
                    }
                    .background(bg)
                    .testTag("lifted-task")
                    .clearAndSetSemantics {},
            ) { TaskRow(drag.row.task, OverlayActions) }
        }
    }
}

internal fun LazyListScope.sortItems(
    entries: List<SortItem>,
    draw: @Composable (SortItem, Modifier) -> Unit,
) {
    items(entries, key = { it.key }, contentType = { it is SortItem.Row }) { entry ->
        draw(entry, Modifier.animateItem(fadeInSpec = null, placementSpec = tween(175), fadeOutSpec = null))
    }
}

@Composable
private fun reorderActions(row: SortItem.Row, entries: List<SortItem>, place: PlaceTask): Modifier {
    val list = row.list ?: return Modifier
    if (row.task.done) return Modifier
    val siblings = entries.filterIsInstance<SortItem.Row>().filter { it.list == list && !it.task.done }
    val index = siblings.indexOf(row)
    val up = stringResource(R.string.plan_move_up)
    val down = stringResource(R.string.plan_move_down)
    return Modifier.semantics {
        customActions = buildList {
            if (index > 0) add(CustomAccessibilityAction(up) {
                place(row.task.id, list, siblings[index - 1].task.id)
                true
            })
            if (index in 0 until siblings.lastIndex) add(CustomAccessibilityAction(down) {
                place(row.task.id, list, siblings.getOrNull(index + 2)?.task?.id)
                true
            })
        }
    }
}

private val RowShape = RoundedCornerShape(10.dp)
private val OverlayActions = RowActions({}, {}, {}, {})
