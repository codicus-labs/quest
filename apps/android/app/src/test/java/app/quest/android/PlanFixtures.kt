package app.quest.android

import app.quest.android.core.PlanRules
import uniffi.quest_mobile.HomeTask
import uniffi.quest_mobile.Moment
import uniffi.quest_mobile.MoveTarget
import uniffi.quest_mobile.PlanData
import uniffi.quest_mobile.PlanDay
import uniffi.quest_mobile.PlanWindow

/** A plan around Monday 2026-11-23 (today): Saturday 21 to Friday 27. */
object PlanSamples {
    private fun task(id: String, title: String, quest: uniffi.quest_mobile.QuestChip?, done: Boolean = false, tracked: String? = null) =
        HomeTask(id = id, title = title, done = done, quest = quest, tracked = tracked, timerRunning = false, canStartTimer = !done)

    private val names = listOf("Saturday" to "Sat", "Sunday" to "Sun", "Monday" to "Mon", "Tuesday" to "Tue", "Wednesday" to "Wed", "Thursday" to "Thu", "Friday" to "Fri")

    fun day(number: Int, questWork: List<HomeTask> = emptyList(), other: List<HomeTask> = emptyList()): PlanDay {
        val (name, short) = names[number - 21]
        return PlanDay(
            dayIso = "2026-11-$number",
            weekday = short,
            dayNumber = "$number",
            label = "$name, November $number",
            monthLabel = "November 2026",
            isToday = number == 23,
            isPast = number < 23,
            questWork = questWork,
            other = other,
        )
    }

    val window = PlanWindow(firstDay = "2026-11-21", count = 7u)

    val days = listOf(
        day(21, questWork = listOf(task("p0", "Pick three motifs", Samples.writing, done = true, tracked = "40:00"))),
        day(22),
        day(23, questWork = Samples.home.questWork, other = Samples.home.other),
        day(24, questWork = listOf(task("p1", "Sketch the contact sheet", Samples.writing), task("p2", "Interval run", Samples.fitness))),
        day(25, other = listOf(task("p3", "Water the plants", null))),
        day(26, questWork = listOf(task("p4", "Edit chapter 3", Samples.writing))),
        day(27),
    )

    val plan = PlanData(window = window, days = days, todayIso = "2026-11-23", questFilter = null, quests = Samples.home.quests, toast = null)

    val targets = listOf(
        MoveTarget("Today", "2026-11-23"),
        MoveTarget("Tomorrow", "2026-11-24"),
        MoveTarget("Wednesday, Nov 25", "2026-11-25"),
        MoveTarget("Backlog · someday", null),
    )
}

/**
 * Plan rules for tests: a window of [count] days starting at the first visible day (today:
 * 2026-11-23) unless [built] already starts there; edge speed is [speed].
 */
class FakePlanRules(var count: UInt = 7u, var speed: Float = 0f) : PlanRules {
    val windows = mutableListOf<String?>()

    override fun window(firstVisible: String?, visibleDays: Int, built: PlanWindow?, now: Moment): PlanWindow {
        windows += firstVisible
        val first = firstVisible ?: "2026-11-23"
        if (built != null && built.firstDay <= first) return built
        return PlanWindow(first, count)
    }

    override fun edgeSpeed(pos: Float, start: Float, end: Float) = speed
}
