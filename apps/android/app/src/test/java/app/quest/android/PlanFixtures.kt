package app.quest.android

import app.quest.android.core.PlanPage
import java.time.LocalDate
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.HomeTask
import uniffi.quest_mobile.PlanDaySection
import uniffi.quest_mobile.PlanDueMarker
import uniffi.quest_mobile.PlanMonthSection
import uniffi.quest_mobile.PlanSections
import uniffi.quest_mobile.PlanWeek
import uniffi.quest_mobile.PlanWeekDay
import uniffi.quest_mobile.QuestChip

object PlanSamples {
    const val today = "2026-11-25"
    const val thursday = "2026-11-26"
    const val friday = "2026-11-27"

    private fun task(id: String, title: String, quest: QuestChip?, date: String? = null) = HomeTask(
        id, title, false, quest, null, false, true,
        date?.let { DayValue(it, it) }, null, false, Samples.notPostponed,
    )

    private val days = List(14) { LocalDate.parse(today).plusDays(it.toLong()).toString() }.map { date ->
        when (date) {
            today -> PlanDaySection(date, Samples.home.questWork.filterNot { it.done }, emptyList())
            thursday -> PlanDaySection(date, listOf(
                task("p1", "Sketch the contact sheet", Samples.writing, date),
                task("p2", "Interval run", Samples.fitness, date),
            ), emptyList())
            friday -> PlanDaySection(date, listOf(task("p3", "Water the plants", null, date)), listOf(PlanDueMarker("p2", "Interval run")))
            else -> PlanDaySection(date, emptyList(), emptyList())
        }
    }

    private val weeks = List(21) { LocalDate.parse("2026-11-23").plusDays(it.toLong()).toString() }.chunked(7).map { dates ->
        PlanWeek(dates.map { date ->
            val section = days.find { it.date == date }
            PlanWeekDay(date, section != null && (section.tasks.isNotEmpty() || section.due.isNotEmpty()), section != null)
        })
    }

    val plan = PlanSections(
        days,
        listOf(
            PlanMonthSection("2026-12-01", true, listOf(PlanDaySection("2026-12-18", emptyList(), emptyList()))),
            PlanMonthSection("2027-01-01", false, listOf(PlanDaySection("2027-01-07", listOf(task("p4", "Edit chapter 3", Samples.writing, "2027-01-07")), emptyList()))),
        ),
        Samples.home.questWork.filter { it.done },
        listOf(task("p5", "Choose a cover photo", Samples.writing), task("p6", "Check the race route", Samples.fitness)),
        weeks,
    )

    val page = PlanPage(plan, ListSamples.todayIdle)
}
