package app.quest.android.ui.plan

import app.quest.android.PlanSamples
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.quest_mobile.TaskSection

class PlanRowsTest {
    private val rows = planRows(PlanSamples.days, showOther = true)
    private val keys = rows.map { it.key }

    /** Every row laid out 100 px high, from the row at [first] on. */
    private fun laidOut(first: Int = 0, last: Int = rows.lastIndex) = (first..last).map { RowBounds(it, (it - first) * 100, (it - first) * 100 + 100) }

    private fun card(id: String) = rows.filterIsInstance<PlanRow.Card>().first { it.task.id == id }

    private fun dragged(id: String) = card(id).let { DraggedCard(it.task, it.day, it.section) }

    private fun top(key: String) = keys.indexOf(key) * 100f

    @Test
    fun daysHaveHeadAddQuestWorkThenOther() {
        val monday = rows.filter { it.day == "2026-11-23" }.map { it.key }
        assertEquals(
            listOf("head:2026-11-23", "add:2026-11-23", "card:t1", "card:t4", "card:t2", "other:2026-11-23", "card:t3"),
            monday,
        )
    }

    @Test
    fun pastDaysHaveNoAddFieldAndHiddenOtherHasNoDivider() {
        val hidden = planRows(PlanSamples.days, showOther = false)
        assertFalse(hidden.any { it is PlanRow.Add && it.day == "2026-11-21" })
        assertFalse(hidden.any { it is PlanRow.OtherDivider || (it as? PlanRow.Card)?.section == TaskSection.OTHER })
        assertTrue(rows.filterIsInstance<PlanRow.Card>().first { it.day == "2026-11-21" }.past)
    }

    @Test
    fun aDropOnTheUpperHalfOfACardGoesBeforeIt() {
        val target = dropTarget(rows, laidOut(), top("card:p2") + 30f, dragged("t1"))!!

        assertEquals(DropTarget("2026-11-24", "p2", DropLine("card:p2", above = true)), target)
    }

    @Test
    fun aDropOnTheLowerHalfGoesBeforeTheNextCardOrToTheEnd() {
        val target = dropTarget(rows, laidOut(), top("card:p2") + 70f, dragged("t1"))!!

        assertEquals(DropTarget("2026-11-24", null, DropLine("card:p2", above = false)), target)
    }

    @Test
    fun questWorkOnADayWithoutQuestWorkLandsUnderTheAddField() {
        val target = dropTarget(rows, laidOut(), top("card:p3") + 10f, dragged("t1"))!!

        // Wednesday has only OTHER: quest work goes in its own section, under the add field.
        assertEquals(DropTarget("2026-11-25", null, DropLine("add:2026-11-25", above = false)), target)
    }

    @Test
    fun otherWorkKeepsToOtherCards() {
        val target = dropTarget(rows, laidOut(), top("card:t1") + 10f, dragged("p3"))!!

        assertEquals(DropTarget("2026-11-23", "t3", DropLine("card:t3", above = true)), target)
    }

    @Test
    fun cardsOffScreenCountByTheirPlace() {
        // Only Tuesday's head is laid out; its cards come after it, below the screen.
        val head = keys.indexOf("head:2026-11-24")
        val target = dropTarget(rows, listOf(RowBounds(head, 0, 100)), 50f, dragged("t1"))!!

        assertEquals("p1", target.beforeTaskId)
    }

    @Test
    fun cardsScrolledOffAboveAPinnedHeadStayAbove() {
        // Monday's head is pinned at the top; t1 has scrolled away under it, t4 and t2 show.
        val head = RowBounds(keys.indexOf("head:2026-11-23"), 0, 100)
        val shown = listOf("card:t4", "card:t2").mapIndexed { i, key -> RowBounds(keys.indexOf(key), 100 + i * 100, 200 + i * 100) }

        val target = dropTarget(rows, listOf(head) + shown, 120f, dragged("p1"))!!

        assertEquals("t4", target.beforeTaskId)
    }

    @Test
    fun nothingLaidOutMeansNoTarget() {
        assertNull(dropTarget(rows, emptyList(), 0f, dragged("t1")))
    }

    @Test
    fun droppingRightBeforeItsNextCardChangesNothing() {
        val t1 = dragged("t1")
        assertTrue(isNoMove(rows, t1, DropTarget("2026-11-23", "t4", DropLine("card:t4", true))))
        assertFalse(isNoMove(rows, t1, DropTarget("2026-11-23", "t2", DropLine("card:t2", true))))
        assertFalse(isNoMove(rows, t1, DropTarget("2026-11-24", "p1", DropLine("card:p1", true))))
        assertTrue(isNoMove(rows, dragged("t2"), DropTarget("2026-11-23", null, DropLine("card:t2", false))))
    }
}
