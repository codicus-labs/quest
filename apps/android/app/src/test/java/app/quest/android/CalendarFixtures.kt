package app.quest.android

import uniffi.quest_mobile.CalendarData
import uniffi.quest_mobile.CalendarDay
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.EntryCheck
import uniffi.quest_mobile.EntryDraft
import uniffi.quest_mobile.FormError
import uniffi.quest_mobile.FormField
import uniffi.quest_mobile.GridBlockKind
import uniffi.quest_mobile.GridPlacement
import uniffi.quest_mobile.LogWorkCheck
import uniffi.quest_mobile.LogWorkDraft
import uniffi.quest_mobile.TaskChoices
import uniffi.quest_mobile.TaskOption

/** Calendar pages around Monday 2026-11-23 (today, 09:30), with the forms' samples. */
object CalendarSamples {
    private val weekdays = listOf("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")

    private fun day(number: Int): CalendarDay {
        val iso = "2026-11-$number"
        val today = number == 23
        val blocks = when (number) {
            23 -> Samples.grid.blocks
            24 -> listOf(
                gridBlock("e5", GridBlockKind.SESSION, 10 * 60, 11 * 60 + 30, "Edit chapter 2", "10:00 – 11:30", Samples.writing.colorHex),
                gridBlock("p2", GridBlockKind.PLANNED, 17 * 60, 18 * 60, "Interval run", "17:00 – 18:00", Samples.fitness.colorHex),
            )
            22 -> listOf(gridBlock("e4", GridBlockKind.SESSION, 9 * 60, 10 * 60, "Call the bank", "09:00 – 10:00", null))
            else -> emptyList()
        }
        return CalendarDay(
            grid = Samples.grid.copy(day = iso, dayLabel = "November $number", blocks = blocks, nowMinute = if (today) 9f * 60f + 30f else null),
            weekday = weekdays[(number - 23 + 7) % 7],
            dayNumber = "$number",
            isToday = today,
            total = when (number) {
                23 -> "2.5h"
                24 -> "1.5h"
                22 -> "1h"
                else -> "0h"
            },
        )
    }

    /** The page with today: 1 day (today), 3 days (from yesterday) or the week from Monday. */
    fun page(dayCount: Int): CalendarData {
        val first = when (dayCount) {
            1 -> 23
            3 -> 22
            else -> 23
        }
        return CalendarData(
            days = (first until first + dayCount).map(::day),
            previousFirstDay = "2026-11-${first - dayCount}",
            nextFirstDay = "2026-11-${first + dayCount}",
            showsToday = true,
            weekLabel = if (dayCount == 3) "Nov 22 – 24" else "Nov 23 – 29",
            weekHours = "6.2h",
            weeklyTarget = "20h",
            weekProgress = 0.31f,
            scrollMinute = 9u * 60u,
            monthLabel = "November 2026",
            toast = null,
        )
    }

    val placement = GridPlacement(day = "2026-11-23", startMinute = 10u * 60u, endMinute = 11u * 60u + 15u, rangeLabel = "10:00 – 11:15")

    val logWorkDraft = LogWorkDraft(taskId = null, newTitle = "", questId = null, day = "2026-11-23", startMinute = 9u * 60u + 15u, endMinute = 9u * 60u + 30u)

    private val today = DayValue(iso = "2026-11-23", label = "Nov 23, 2026")

    val logWorkCheck = LogWorkCheck(
        date = today,
        startLabel = "09:15",
        endLabel = "09:30",
        duration = "15m",
        taskTitle = null,
        isNewTask = false,
        quest = null,
        // No task and no title yet: nothing to choose a quest for.
        canChooseQuest = false,
        questOptions = Samples.home.quests,
        error = FormError(FormField.TASK, "Choose a task or type a title."),
    )

    /** Log work for an existing task: its quest shows, read-only. */
    val logWorkExistingTask = logWorkCheck.copy(taskTitle = "Morning run", quest = Samples.fitness, error = null)

    /** Log work for a new task with a quest chosen; the end is before the start. */
    val logWorkNewTaskError = logWorkCheck.copy(
        taskTitle = "Plan the workshop",
        isNewTask = true,
        quest = Samples.writing,
        canChooseQuest = true,
        startLabel = "14:00",
        endLabel = "13:30",
        duration = "–",
        error = FormError(FormField.END, "The end must be after the start."),
    )

    val entryDraft = EntryDraft(
        entryId = "e1",
        taskId = "t2",
        startDay = "2026-11-23",
        startMinute = 7u * 60u,
        endDay = "2026-11-23",
        endMinute = 8u * 60u + 15u,
        endsOtherDay = false,
    )

    val entryCheck = EntryCheck(
        running = false,
        taskTitle = "Morning run",
        quest = Samples.fitness,
        canOpenTask = true,
        startDate = today,
        startLabel = "07:00",
        endDate = today,
        endLabel = "08:15",
        duration = "1h 15m",
        error = null,
    )

    /** Over midnight: ends the next day. */
    val entryOverMidnight = entryCheck.copy(
        taskTitle = "Edit chapter 2",
        quest = Samples.writing,
        startLabel = "23:00",
        endDate = DayValue(iso = "2026-11-24", label = "Nov 24, 2026"),
        endLabel = "00:45",
        duration = "1h 45m",
    )

    /** Over midnight with the end date before the start date. */
    val entryEndDateError = entryOverMidnight.copy(
        endDate = DayValue(iso = "2026-11-22", label = "Nov 22, 2026"),
        duration = "–",
        error = FormError(FormField.END_DATE, "The end date can't be before the start date."),
    )

    val choices = TaskChoices(
        options = listOf(
            TaskOption("t1", "Outline chapter 3", false, "Open", Samples.writing),
            TaskOption("t4", "Edit chapter 2", false, "Open", Samples.writing),
            TaskOption("t3", "Call the bank", false, "Open", null),
            TaskOption("t2", "Morning run", true, "Done", Samples.fitness),
        ),
        createLabel = null,
    )
}
