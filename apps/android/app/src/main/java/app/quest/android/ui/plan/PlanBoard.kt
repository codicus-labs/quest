package app.quest.android.ui.plan

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.gestures.snapping.SnapPosition
import androidx.compose.foundation.gestures.snapping.rememberSnapFlingBehavior
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import kotlinx.coroutines.flow.distinctUntilChanged
import uniffi.quest_mobile.PlanDay

/** Columns narrower than this make way for fewer, wider ones (desktop shows five; touch cards need more room). */
private val MinColumnWidth = 260.dp
private const val MAX_COLUMNS = 5
private val ColumnGap = QuestSpacing.medium

/**
 * Medium and expanded plan: day columns side by side (desktop), scrolling sideways without end and
 * snapping to a column. Each column scrolls its own cards. The core builds a window of days as
 * on the compact list.
 */
@Composable
fun PlanBoard(state: PlanUiState.Ready, actions: PlanActions, onMonth: (String) -> Unit, modifier: Modifier = Modifier) {
    BoxWithConstraints(modifier.fillMaxSize()) {
        val usable = maxWidth - ColumnGap
        val columns = (usable / (MinColumnWidth + ColumnGap)).toInt().coerceIn(2, MAX_COLUMNS)
        val columnWidth = (usable - ColumnGap * columns) / columns
        Board(state, actions, onMonth, columns, columnWidth)
    }
}

@Composable
private fun Board(state: PlanUiState.Ready, actions: PlanActions, onMonth: (String) -> Unit, columns: Int, columnWidth: Dp) {
    val days = state.plan.days
    val showOther = state.prefs.showOther
    val rowState = rememberLazyListState()
    val drag = remember { PlanDragState() }
    val registry = remember { ColumnRegistry() }
    val density = LocalDensity.current
    val rowsByDay = remember(days, showOther) { days.associate { it.dayIso to dayRows(it, showOther) } }
    val surface = BoardSurface(rowState, registry, rowsByDay, density, actions.edgeSpeed)
    val dragModifier = rememberPlanDrag(drag, surface, dropCallbacks(actions))

    BoardScrollRequests(state, rowState, columns, actions)
    VisibleColumns(rowState, drag, days, columns, actions, onMonth)

    Box(
        Modifier
            .fillMaxSize()
            .onGloballyPositioned { registry.origin = it.positionInRoot() }
            .then(dragModifier),
    ) {
        LazyRow(
            state = rowState,
            flingBehavior = rememberSnapFlingBehavior(rowState, SnapPosition.Start),
            contentPadding = PaddingValues(start = ColumnGap, end = ColumnGap),
            horizontalArrangement = Arrangement.spacedBy(ColumnGap),
            modifier = Modifier.fillMaxSize().testTag("plan-board"),
        ) {
            items(days, key = { it.dayIso }, contentType = { "day" }) { day ->
                DayColumn(day, rowsByDay[day.dayIso].orEmpty(), state, drag, registry, actions, Modifier.width(columnWidth))
            }
        }
        DraggedCardOverlay(drag)
    }
}

/** One day: its head, then its own scrolling list of add field and cards. */
@Composable
private fun DayColumn(day: PlanDay, rows: List<PlanRow>, state: PlanUiState.Ready, drag: PlanDragState, registry: ColumnRegistry, actions: PlanActions, modifier: Modifier) {
    val colors = QuestTheme.colors
    val listState = rememberLazyListState()
    val iso = day.dayIso
    DisposableEffect(iso, listState) {
        registry.lists[iso] = listState
        onDispose {
            registry.lists.remove(iso)
            registry.bounds.remove(iso)
        }
    }
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    Column(
        modifier
            .fillMaxHeight()
            .padding(bottom = QuestSpacing.small)
            .alpha(if (day.isPast) 0.8f else 1f)
            .background(if (day.isToday) colors.card.copy(alpha = 0.4f) else colors.bg, shape)
            .border(QuestSpacing.hairline, colors.border.copy(alpha = 0.5f), shape)
            .padding(horizontal = QuestSpacing.small)
            .padding(top = QuestSpacing.medium)
            .testTag("plan-column:$iso"),
    ) {
        ColumnHead(day, Modifier.padding(bottom = QuestSpacing.medium))
        LazyColumn(
            state = listState,
            verticalArrangement = Arrangement.spacedBy(QuestSpacing.listGap),
            modifier = Modifier
                .fillMaxSize()
                .onGloballyPositioned { registry.bounds[iso] = it.boundsInRoot().translate(-registry.origin) },
        ) {
            items(rows, key = { it.key }, contentType = { it::class }) { row -> PlanRowContent(row, state, drag, actions) }
            item(key = "end:$iso", contentType = "end") { ListEnd() }
        }
    }
}

/** Puts today in the second column (first with two), as the desktop's Today keeps a day before it in view; a picked date goes first. */
@Composable
private fun BoardScrollRequests(state: PlanUiState.Ready, rowState: LazyListState, columns: Int, actions: PlanActions) {
    val request = state.scroll ?: return
    LaunchedEffect(request, state.plan.days) {
        val day = request.day ?: state.plan.todayIso
        val index = state.plan.days.indexOfFirst { it.dayIso == day }
        if (index < 0) return@LaunchedEffect
        val first = if (request.day == null && columns > 2) (index - 1).coerceAtLeast(0) else index
        if (request.animate) rowState.animateScrollToItem(first) else rowState.scrollToItem(first)
        actions.scrolled(request)
    }
}

/** Reports the columns on screen and the middle one's month (desktop); not while dragging. */
@Composable
private fun VisibleColumns(rowState: LazyListState, drag: PlanDragState, days: List<PlanDay>, columns: Int, actions: PlanActions, onMonth: (String) -> Unit) {
    val currentDays by rememberUpdatedState(days)
    LaunchedEffect(rowState, drag) {
        snapshotFlow {
            if (drag.active) return@snapshotFlow null
            val visible = rowState.layoutInfo.visibleItemsInfo.mapNotNull { info -> info.key as? String }
            visible.firstOrNull()?.let { first -> Triple(first, visible.size.coerceAtMost(columns + 1), visible.getOrNull(visible.size / 2)) }
        }.distinctUntilChanged().collect { range ->
            val (first, count, middle) = range ?: return@collect
            actions.visibleRange(first, count)
            currentDays.firstOrNull { it.dayIso == middle }?.let { onMonth(it.monthLabel) }
        }
    }
}

/** Where each column's card list is (board pixels) and its scroll state; columns register themselves. */
private class ColumnRegistry {
    var origin = Offset.Zero
    val lists = mutableMapOf<String, LazyListState>()
    val bounds = mutableMapOf<String, Rect>()
}

/** The board as a drag surface: the column under the finger, then its rows as in the compact list. */
private class BoardSurface(
    private val row: LazyListState,
    private val registry: ColumnRegistry,
    private val rowsByDay: Map<String, List<PlanRow>>,
    private val density: Density,
    private val edgeSpeed: (Float, Float, Float) -> Float,
) : DragSurface {
    /** The column under [x], else the nearest one. */
    private fun columnAt(x: Float): Pair<String, Rect>? =
        registry.bounds.entries.minByOrNull { (_, r) -> if (x in r.left..r.right) 0f else minOf(kotlin.math.abs(x - r.left), kotlin.math.abs(x - r.right)) }?.toPair()

    private fun laidOut(day: String, rows: List<PlanRow>): List<RowBounds> {
        val list = registry.lists[day] ?: return emptyList()
        val indexByKey = rows.withIndex().associate { (index, r) -> r.key to index }
        return list.layoutInfo.visibleItemsInfo.mapNotNull { item ->
            val index = indexByKey[item.key] ?: return@mapNotNull null
            RowBounds(index, item.offset, item.offset + item.size)
        }
    }

    override fun cardAt(point: Offset): Grab? {
        val (day, bounds) = registry.bounds.entries.firstOrNull { it.value.contains(point) }?.toPair() ?: return null
        val rows = rowsByDay[day] ?: return null
        val y = point.y - bounds.top
        val list = registry.lists[day] ?: return null
        val item = list.layoutInfo.visibleItemsInfo.firstOrNull { y.toInt() in it.offset until it.offset + it.size } ?: return null
        val card = rows.firstOrNull { it.key == item.key } as? PlanRow.Card ?: return null
        return Grab(card, Offset(bounds.left, bounds.top + item.offset), IntSize(bounds.width.toInt(), item.size))
    }

    override fun targetAt(point: Offset, dragged: DraggedCard): DropTarget? {
        val (day, bounds) = columnAt(point.x) ?: return null
        val rows = rowsByDay[day] ?: return null
        return dropTarget(rows, laidOut(day, rows), point.y - bounds.top, dragged)
    }

    override fun rowsOf(day: String) = rowsByDay[day].orEmpty()

    override suspend fun autoScroll(point: Offset, seconds: Float): Boolean {
        val dp = density.density
        val width = row.layoutInfo.viewportSize.width
        val sideways = edgeSpeed(point.x / dp, 0f, width / dp)
        if (sideways != 0f) row.scrollBy(sideways * seconds * dp)
        val (day, bounds) = columnAt(point.x) ?: return sideways != 0f
        val down = edgeSpeed(point.y / dp, bounds.top / dp, bounds.bottom / dp)
        if (down != 0f) registry.lists[day]?.scrollBy(down * seconds * dp)
        return sideways != 0f || down != 0f
    }
}
