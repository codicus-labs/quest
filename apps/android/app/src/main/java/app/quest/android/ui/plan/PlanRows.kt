package app.quest.android.ui.plan

import androidx.compose.runtime.Immutable
import uniffi.quest_mobile.HomeTask
import uniffi.quest_mobile.PlanDay
import uniffi.quest_mobile.TaskSection

/**
 * One row of a plan list: the compact list holds every day's rows in a row, a day column only its
 * own (without [Head], which the column draws above its list). Rows are layout, not rules: the
 * core already sorted and filtered the cards.
 */
@Immutable
sealed interface PlanRow {
    /** The lazy list key; unique across all days. */
    val key: String
    val day: String

    data class Head(val plan: PlanDay) : PlanRow {
        override val key get() = "head:${plan.dayIso}"
        override val day get() = plan.dayIso
    }

    data class Add(override val day: String) : PlanRow {
        override val key get() = "add:$day"
    }

    /** "OTHER" above a day's other tasks. */
    data class OtherDivider(override val day: String) : PlanRow {
        override val key get() = "other:$day"
    }

    data class Card(override val day: String, val section: TaskSection, val task: HomeTask, val past: Boolean) : PlanRow {
        override val key get() = cardKey(task.id)
    }

    companion object {
        fun cardKey(taskId: String) = "card:$taskId"
    }
}

/** The rows of [days]: head (with [heads]), add field (not on past days), QUEST WORK, then OTHER when shown. */
fun planRows(days: List<PlanDay>, showOther: Boolean, heads: Boolean = true): List<PlanRow> = buildList {
    for (day in days) addAll(dayRows(day, showOther, heads))
}

fun dayRows(day: PlanDay, showOther: Boolean, head: Boolean = false): List<PlanRow> = buildList {
    val iso = day.dayIso
    if (head) add(PlanRow.Head(day))
    if (!day.isPast) add(PlanRow.Add(iso))
    day.questWork.forEach { add(PlanRow.Card(iso, TaskSection.QUEST_WORK, it, day.isPast)) }
    if (showOther && day.other.isNotEmpty()) {
        add(PlanRow.OtherDivider(iso))
        day.other.forEach { add(PlanRow.Card(iso, TaskSection.OTHER, it, day.isPast)) }
    }
}

/** Where a laid-out row sits along the list's axis, in the list's own pixels. */
data class RowBounds(val index: Int, val top: Int, val bottom: Int)

/** The card being dragged, with where it came from. */
@Immutable
data class DraggedCard(val task: HomeTask, val day: String, val section: TaskSection)

/** Where the drag line shows: on top of the row [key], or below it. */
data class DropLine(val key: String, val above: Boolean)

/** A drop: [task][DraggedCard] to [day] before [beforeTaskId] (null: the end of its section that day). */
data class DropTarget(val day: String, val beforeTaskId: String?, val line: DropLine)

/**
 * The drop under [y] (list pixels) for [dragged]: the day of the row under the finger, and the
 * first card of the dragged card's section that day whose middle lies below the finger. Cards off
 * screen count as below when they come after the last row on screen. Null when nothing is laid out.
 */
fun dropTarget(rows: List<PlanRow>, visible: List<RowBounds>, y: Float, dragged: DraggedCard): DropTarget? {
    if (visible.isEmpty()) return null
    // A sticky head may overlap the row under it; it is the same day.
    val byTop = visible.sortedBy { it.top }
    val hovered = byTop.firstOrNull { y < it.bottom } ?: byTop.last()
    val day = rows.getOrNull(hovered.index)?.day ?: return null
    val byIndex = visible.associateBy { it.index }
    // A pinned sticky head stays in the visible rows, so only cards after the last visible row are below.
    val last = visible.maxOf { it.index }
    val cards = rows.withIndex().filter { (_, row) ->
        row is PlanRow.Card && row.day == day && row.section == dragged.section && row.task.id != dragged.task.id
    }
    val before = cards.firstOrNull { (index, _) ->
        val bounds = byIndex[index]
        if (bounds == null) index > last else (bounds.top + bounds.bottom) / 2f > y
    }
    if (before != null) {
        val card = before.value as PlanRow.Card
        return DropTarget(day, card.task.id, DropLine(card.key, above = true))
    }
    return DropTarget(day, null, DropLine(lastRowBefore(rows, day, dragged, cards.lastOrNull()?.value), above = false))
}

/** The row the "end of section" line sits under: the section's last card, else what comes before the section. */
private fun lastRowBefore(rows: List<PlanRow>, day: String, dragged: DraggedCard, lastCard: PlanRow?): String {
    if (lastCard != null) return lastCard.key
    val dayRows = rows.filter { it.day == day && (it as? PlanRow.Card)?.task?.id != dragged.task.id }
    val beforeSection = when (dragged.section) {
        TaskSection.QUEST_WORK -> dayRows.lastOrNull { it !is PlanRow.Card && it !is PlanRow.OtherDivider }
        TaskSection.OTHER -> dayRows.lastOrNull()
    }
    return (beforeSection ?: dayRows.firstOrNull() ?: rows.first { it.day == day }).key
}

/** Whether dropping [dragged] at [target] leaves it where it is (no command needed). */
fun isNoMove(rows: List<PlanRow>, dragged: DraggedCard, target: DropTarget): Boolean {
    if (target.day != dragged.day) return false
    val section = rows.filterIsInstance<PlanRow.Card>().filter { it.day == dragged.day && it.section == dragged.section }
    val at = section.indexOfFirst { it.task.id == dragged.task.id }
    if (at < 0) return false
    return section.getOrNull(at + 1)?.task?.id == target.beforeTaskId
}
