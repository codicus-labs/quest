package app.quest.android.ui.plan

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntSize
import app.quest.android.ui.theme.QuestSpacing
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * Compact plan: one endless list of days around today, each with a sticky head, its add field,
 * QUEST WORK and (when shown) OTHER. The core builds a window of days; scrolling near its end
 * builds the next one, and the list keeps its place by key.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun PlanList(state: PlanUiState.Ready, actions: PlanActions, onMonth: (String) -> Unit, modifier: Modifier = Modifier) {
    val rows = remember(state.plan.days, state.prefs.showOther) { planRows(state.plan.days, state.prefs.showOther) }
    val listState = rememberLazyListState()
    val drag = remember { PlanDragState() }
    val density = LocalDensity.current
    var height by remember { mutableIntStateOf(0) }
    val surface = ListSurface(listState, rows, density, actions.edgeSpeed) { height }
    val dragModifier = rememberPlanDrag(drag, surface, dropCallbacks(actions))

    ScrollRequests(state, rows, listState, actions)
    VisibleDays(listState, rows, drag, actions, onMonth)

    Box(modifier.fillMaxSize().onSizeChanged { height = it.height }) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().then(dragModifier).testTag("plan-list"),
            // No top padding: drag hit-tests use item offsets as list pixels.
            contentPadding = PaddingValues(start = QuestSpacing.screen, end = QuestSpacing.screen),
            verticalArrangement = Arrangement.spacedBy(QuestSpacing.listGap),
        ) {
            var at = 0
            while (at < rows.size) {
                val head = rows[at] as PlanRow.Head
                val end = (at + 1 until rows.size).firstOrNull { rows[it] is PlanRow.Head } ?: rows.size
                val dayRows = rows.subList(at + 1, end)
                stickyHeader(key = head.key, contentType = "head") { DayHead(head.plan) }
                items(dayRows, key = { it.key }, contentType = { it::class }) { row -> PlanRowContent(row, state, drag, actions) }
                at = end
            }
            item(key = "end", contentType = "end") { ListEnd() }
        }
        DraggedCardOverlay(drag)
    }
}

/** Scrolls to a requested day (Today, a picked date, the day shown before) once it is built. */
@Composable
private fun ScrollRequests(state: PlanUiState.Ready, rows: List<PlanRow>, listState: LazyListState, actions: PlanActions) {
    val request = state.scroll ?: return
    LaunchedEffect(request, rows) {
        val day = request.day ?: state.plan.todayIso
        val index = rows.indexOfFirst { it is PlanRow.Head && it.day == day }
        if (index < 0) return@LaunchedEffect
        if (request.animate) listState.animateScrollToItem(index) else listState.scrollToItem(index)
        actions.scrolled(request)
    }
}

/** Reports the days on screen (window and saved place) and the first one's month; not while dragging. */
@Composable
private fun VisibleDays(listState: LazyListState, rows: List<PlanRow>, drag: PlanDragState, actions: PlanActions, onMonth: (String) -> Unit) {
    val currentRows by rememberUpdatedState(rows)
    LaunchedEffect(listState, drag) {
        snapshotFlow {
            if (drag.active) return@snapshotFlow null
            val byKey = currentRows.associateBy { it.key }
            val days = listState.layoutInfo.visibleItemsInfo.mapNotNull { byKey[it.key]?.day }.distinct()
            days.firstOrNull()?.let { first -> first to days.size }
        }.distinctUntilChanged().collect { range ->
            val (first, count) = range ?: return@collect
            actions.visibleRange(first, count)
            (currentRows.firstOrNull { it is PlanRow.Head && it.day == first } as? PlanRow.Head)?.let { onMonth(it.plan.monthLabel) }
        }
    }
}

/** What a drop or a press without moving does on any plan layout. */
fun dropCallbacks(actions: PlanActions) = DragCallbacks(
    drop = { dragged, target -> actions.moveTask(dragged.task.id, target.day, target.beforeTaskId) },
    menu = { dragged -> actions.openMoveTo(dragged.task.id, dragged.task.title) },
)

/** The compact list as a drag surface: list pixels are the gesture's, rows hit-test by their keys. */
private class ListSurface(
    private val list: LazyListState,
    private val rows: List<PlanRow>,
    private val density: Density,
    private val edgeSpeed: (Float, Float, Float) -> Float,
    private val height: () -> Int,
) : DragSurface {
    private val indexByKey: Map<Any, Int> = rows.withIndex().associate { (index, row) -> row.key to index }

    private fun laidOut(): List<RowBounds> = list.layoutInfo.visibleItemsInfo.mapNotNull { item ->
        val index = indexByKey[item.key] ?: return@mapNotNull null
        RowBounds(index, item.offset, item.offset + item.size)
    }

    override fun cardAt(point: Offset): Grab? {
        val items = list.layoutInfo.visibleItemsInfo
        // A pinned head covers the card under it; pressing the head lifts nothing.
        if (items.any { point.y.toInt() in it.offset until it.offset + it.size && rows.getOrNull(indexByKey[it.key] ?: -1) is PlanRow.Head }) return null
        val item = items.firstOrNull { point.y.toInt() in it.offset until it.offset + it.size && rows.getOrNull(indexByKey[it.key] ?: -1) is PlanRow.Card }
            ?: return null
        val card = rows[indexByKey.getValue(item.key)] as PlanRow.Card
        val side = with(density) { QuestSpacing.screen.toPx() }
        val width = list.layoutInfo.viewportSize.width - 2 * side
        return Grab(card, Offset(side, item.offset.toFloat()), IntSize(width.toInt(), item.size))
    }

    override fun targetAt(point: Offset, dragged: DraggedCard) = dropTarget(rows, laidOut(), point.y, dragged)

    override fun rowsOf(day: String) = rows.filter { it.day == day }

    override suspend fun autoScroll(point: Offset, seconds: Float): Boolean {
        val dp = density.density
        val speed = edgeSpeed(point.y / dp, 0f, height() / dp)
        if (speed == 0f) return false
        list.scrollBy(speed * seconds * dp)
        return true
    }
}
