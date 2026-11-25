package app.quest.android

import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.EntryCheck
import uniffi.quest_mobile.EntryDraft
import uniffi.quest_mobile.ListKind
import uniffi.quest_mobile.ListRow
import uniffi.quest_mobile.ListSection
import uniffi.quest_mobile.MarkTone
import uniffi.quest_mobile.PlayerCard
import uniffi.quest_mobile.QuestChip
import uniffi.quest_mobile.QuestNav
import uniffi.quest_mobile.QuestNavRow
import uniffi.quest_mobile.RowMark
import uniffi.quest_mobile.SessionKind
import uniffi.quest_mobile.TaskList
import uniffi.quest_mobile.TrackedBlock
import uniffi.quest_mobile.TrackedData
import uniffi.quest_mobile.WeekQuestLine
import uniffi.quest_mobile.WeekSummary

/** The redesigned lists, Quests tab, Tracked tab and player on Monday 2026-11-23 (today, 09:30). */
object ListSamples {
    val launch = QuestChip("q3", "Launch my idea", "#d9625c")
    val writing = Samples.writing
    val fitness = Samples.fitness

    private fun row(id: String, title: String, quest: QuestChip? = null, vararg marks: RowMark) =
        ListRow(id, title, done = false, quest = quest, marks = marks.toList(), timerRunning = false, timer = null, canStartTimer = true, checklistProgress = null)

    private fun done(id: String, title: String, quest: QuestChip? = null) =
        ListRow(id, title, done = true, quest = quest, marks = emptyList(), timerRunning = false, timer = null, canStartTimer = false, checklistProgress = null)

    private fun overdue(day: String) = RowMark("overdue · $day", MarkTone.OVERDUE)
    private fun since(day: String) = RowMark("since $day", MarkTone.MUTED)
    private val dueToday = RowMark("due today", MarkTone.ACCENT)

    val runningRow = row("t5", "Design the next chapter", writing, RowMark("due Fri", MarkTone.MUTED)).copy(timerRunning = true, timer = "36:13")

    val todayOpen = listOf(
        row("t1", "Sketch a pricing page", launch, overdue("Sun 22")),
        row("t2", "Call the landlord", null, overdue("Sat 21")),
        row("t3", "Draft this week’s essay", writing, since("Sun 22"), dueToday),
        row("t4", "Write the launch announcement", launch, since("Sat 21"), RowMark("due Thu", MarkTone.MUTED)),
        runningRow,
        row("t6", "Easy 5 km. Leave the headphones at home.", fitness),
    )

    val today = TaskList(
        kind = ListKind.Today,
        title = "Today",
        subtitle = "Monday, November 23 · 3.9h tracked",
        quest = null,
        ring = 0f,
        sections = listOf(
            ListSection(null, todayOpen),
            ListSection("2 done", listOf(done("t7", "Read a chapter, take a few notes", fitness), done("t8", "Map the ideas worth pursuing", launch))),
        ),
        empty = false,
        toast = null,
    )

    /** Today without a running timer. */
    val todayIdle = today.copy(sections = today.sections.map { s -> s.copy(rows = s.rows.map { it.copy(timerRunning = false, timer = null) }) })

    val inbox = TaskList(
        kind = ListKind.Inbox,
        title = "Inbox",
        subtitle = "",
        quest = null,
        ring = 0f,
        sections = listOf(
            ListSection(
                null,
                listOf(
                    row("i1", "Pick up the dry cleaning"),
                    row("t2", "Call the landlord", null, overdue("Sat 21")),
                    row("i3", "Newsletter about the launch?"),
                    row("i4", "Try the new running shoes"),
                    row("i5", "Look into “Deep Work” (Anna’s tip)"),
                ),
            ),
        ),
        empty = false,
        toast = null,
    )

    val emptyInbox = inbox.copy(sections = listOf(ListSection(null, emptyList())), empty = true)

    val questList = TaskList(
        kind = ListKind.Quest("q3"),
        title = "Launch my idea",
        subtitle = "8.9h this week · 3 open",
        quest = launch,
        ring = 0.45f,
        sections = listOf(
            ListSection(
                null,
                listOf(
                    row("t1", "Sketch a pricing page", null, overdue("Sun 22")),
                    row("t4", "Write the launch announcement", null, since("Sat 21"), RowMark("due Thu", MarkTone.MUTED)),
                    row("q9", "Make the first impression feel effortless", null, RowMark("Wed", MarkTone.MUTED)),
                ),
            ),
            ListSection("1 done", listOf(done("t8", "Map the ideas worth pursuing"))),
        ),
        empty = false,
        toast = null,
    )

    val logbook = TaskList(
        kind = ListKind.Logbook,
        title = "Logbook",
        subtitle = "",
        quest = null,
        ring = 0f,
        sections = listOf(
            ListSection("Monday, November 23", listOf(done("t7", "Read a chapter, take a few notes", fitness), done("t8", "Map the ideas worth pursuing", launch))),
            ListSection("Sunday, November 22", listOf(done("l3", "Book the venue", launch))),
        ),
        empty = false,
        toast = null,
    )

    val questNav = QuestNav(
        active = listOf(
            QuestNavRow("q3", "Launch my idea", launch.colorHex, "8.9h", 0.45f, "Launch my idea, 8.9h this week"),
            QuestNavRow("q1", "Write the book", writing.colorHex, "1.9h", 0.1f, "Write the book, 1.9h this week"),
            QuestNavRow("q2", "Fitness", fitness.colorHex, "2.3h", 0.12f, "Fitness, 2.3h this week"),
        ),
        completed = listOf(QuestNavRow("q4", "Old hobby", "#a77bd4", "0h", 0f, "Old hobby, 0h this week")),
    )

    val week = WeekSummary(
        total = "12.8h / 20h",
        progress = 0.64f,
        quests = listOf(
            WeekQuestLine("Launch my idea", launch.colorHex, "8.9h", 0.45f),
            WeekQuestLine("Write the book", writing.colorHex, "1.9h", 0.1f),
            WeekQuestLine("Fitness", fitness.colorHex, "2.3h", 0.12f),
        ),
        organizing = "0.4h",
        noQuest = "0.6h",
        postponed = 4u,
    )

    val tracked = TrackedData(
        total = "3.9h",
        firstHour = 8u,
        lastHour = 18u,
        blocks = listOf(
            TrackedBlock("e1", "Map the ideas worth pursuing", launch.colorHex, organizing = false, running = false, startMinute = 9u * 60u, endMinute = 10u * 60u + 30u, range = "09:00–10:30", outsideSinceMinute = null, rebooked = false),
            TrackedBlock("e2", "Organizing", null, organizing = true, running = false, startMinute = 10u * 60u + 30u, endMinute = 10u * 60u + 50u, range = "10:30–10:50", outsideSinceMinute = null, rebooked = false),
            TrackedBlock("e3", "Easy 5 km", fitness.colorHex, organizing = false, running = false, startMinute = 11u * 60u, endMinute = 12u * 60u, range = "11:00–12:00", outsideSinceMinute = null, rebooked = false),
            TrackedBlock("e4", "Design the next chapter", writing.colorHex, organizing = false, running = true, startMinute = 14u * 60u, endMinute = 14u * 60u + 36u, range = "14:00–now", outsideSinceMinute = null, rebooked = false),
        ),
        nowMinute = 14u * 60u + 36u,
        week = week,
    )

    val playerRunning = PlayerCard.Running("t5", "Design the next chapter", writing, "36:13")
    val playerOrganizing = PlayerCard.Organizing("04:12", "t5", "Design the next chapter")
    val playerResume = PlayerCard.Resume("t5", "Design the next chapter", writing)

    private val monday = DayValue("2026-11-23", "Nov 23, 2026")

    val entryDraft = EntryDraft(
        entryId = "e3",
        taskId = "t6",
        startDay = "2026-11-23",
        startMinute = 11u * 60u,
        endDay = "2026-11-23",
        endMinute = 12u * 60u,
        endsOtherDay = false,
    )

    val entryCheck = EntryCheck(
        kind = SessionKind.TASK,
        running = false,
        taskTitle = "Easy 5 km",
        quest = fitness,
        canOpenTask = true,
        startDate = monday,
        startLabel = "11:00",
        endDate = monday,
        endLabel = "12:00",
        duration = "1h",
        error = null,
    )

    val organizingCheck = entryCheck.copy(kind = SessionKind.ORGANIZING, taskTitle = null, quest = null, canOpenTask = false, startLabel = "10:30", endLabel = "10:50", duration = "20m")
}
