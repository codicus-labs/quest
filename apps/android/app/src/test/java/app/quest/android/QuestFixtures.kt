package app.quest.android

import uniffi.quest_mobile.AppView
import uniffi.quest_mobile.ArchiveData
import uniffi.quest_mobile.ArchiveGroup
import uniffi.quest_mobile.ArchiveItem
import uniffi.quest_mobile.HomeTask
import uniffi.quest_mobile.HueOption
import uniffi.quest_mobile.MilestoneRow
import uniffi.quest_mobile.QuestCard
import uniffi.quest_mobile.QuestDetail
import uniffi.quest_mobile.QuestHue
import uniffi.quest_mobile.QuestTaskGroup
import uniffi.quest_mobile.QuestsData
import uniffi.quest_mobile.ReviewData
import uniffi.quest_mobile.ReviewDay
import uniffi.quest_mobile.ReviewQuest
import uniffi.quest_mobile.ReviewSegment
import uniffi.quest_mobile.ReviewWeek
import uniffi.quest_mobile.SearchKind
import uniffi.quest_mobile.SearchResult
import uniffi.quest_mobile.SearchToken
import uniffi.quest_mobile.TrashKind
import uniffi.quest_mobile.UndoToast

/** Quests, quest page, review, archive and search as the core would build them. */
object QuestSamples {
    private const val BLUE = "#5c8fd9"
    private const val GREEN = "#62b87a"

    private val hueColors = listOf("#d9625c", "#e0904a", "#d6b64a", "#9cc44f", GREEN, "#4fb3a9", BLUE, "#a77bd6", "#d66fa8")

    fun hues(selected: QuestHue) = QuestHue.entries.mapIndexed { index, hue ->
        HueOption(hue = hue, name = hue.name.lowercase().replaceFirstChar { it.uppercase() }, colorHex = hueColors[index], selected = hue == selected)
    }

    val sprint = MilestoneRow(id = "m1", title = "First draft done", dateLabel = "Nov 15, 2026", a11yLabel = "First draft done, Nov 15, 2026")

    val writingCard = QuestCard(
        id = "q1",
        title = "Write the book",
        colorHex = BLUE,
        weekHours = "4.5h",
        totalHours = "62h total",
        openTasks = "3 open tasks",
        a11yLabel = "Write the book, 4.5h this week, 62h total, 3 open tasks",
    )
    val fitnessCard = QuestCard(
        id = "q2",
        title = "Fitness",
        colorHex = GREEN,
        weekHours = "1.7h",
        totalHours = "21h total",
        openTasks = "1 open task",
        a11yLabel = "Fitness, 1.7h this week, 21h total, 1 open task",
    )
    val doneCard = QuestCard(
        id = "q3",
        title = "Learn Rust",
        colorHex = "#a77bd6",
        weekHours = "0h",
        totalHours = "40h total",
        openTasks = "0 open tasks",
        a11yLabel = "Learn Rust, 0h this week, 40h total, 0 open tasks",
    )

    val quests = QuestsData(
        allTimeHours = "123h",
        inProgress = "02",
        completedCount = "01",
        active = listOf(writingCard, fitnessCard),
        completed = listOf(doneCard),
        activeLabel = "Active · 2",
        completedLabel = "Completed · 1",
        newQuestHues = hues(QuestHue.TEAL),
        toast = null,
    )

    val emptyQuests = quests.copy(allTimeHours = "0h", inProgress = "00", completedCount = "00", active = emptyList(), completed = emptyList(), activeLabel = "Active · 0", completedLabel = "Completed · 0")

    /** [quests] after the core moved [questId] between Active and Completed: cards keep their hours. */
    fun toggled(quests: QuestsData, questId: String): QuestsData {
        val active = quests.active.find { it.id == questId }
        val completed = quests.completed.find { it.id == questId }
        val card = active ?: completed ?: return quests
        val verb = if (active != null) "Completed" else "Reopened"
        val toast = UndoToast(text = "$verb quest “${card.title}”", actionLabel = "Undo", expiresAtMs = NOW_MS + 6_000)
        val moved = if (active != null) listed(quests, quests.active - card, listOf(card) + quests.completed) else listed(quests, quests.active + card, quests.completed - card)
        return moved.copy(toast = toast)
    }

    /** [quests] with these lists, their tab labels and figures. */
    fun listed(quests: QuestsData, active: List<QuestCard>, completed: List<QuestCard>) = quests.copy(
        active = active,
        completed = completed,
        activeLabel = "Active · ${active.size}",
        completedLabel = "Completed · ${completed.size}",
        inProgress = "%02d".format(active.size),
        completedCount = "%02d".format(completed.size),
    )

    /** The quest page after Complete quest / Reopen quest. */
    fun toggled(detail: QuestDetail) = if (detail.completed) {
        detail.copy(completed = false, statusAction = "Complete quest", badge = null)
    } else {
        detail.copy(completed = true, statusAction = "Reopen quest", badge = "Completed")
    }

    private fun task(id: String, title: String, done: Boolean = false) =
        HomeTask(id = id, title = title, done = done, quest = Samples.writing, tracked = null, timerRunning = false, canStartTimer = !done)

    val detail = QuestDetail(
        id = "q1",
        title = "Write the book",
        description = "A novel about a lighthouse keeper.",
        colorHex = BLUE,
        hues = hues(QuestHue.BLUE),
        completed = false,
        statusAction = "Complete quest",
        badge = null,
        summary = "4.5h this week · 62h total · 2 open tasks",
        weekHours = "4.5h",
        totalHours = "62h",
        openCount = 2u,
        doneCount = 1u,
        taskGroups = listOf(
            QuestTaskGroup("Today", listOf(task("t1", "Outline chapter 3"), task("t4", "Edit chapter 2"))),
            QuestTaskGroup("Done", listOf(task("t5", "Pick a title", done = true))),
        ),
        milestones = listOf(sprint),
        toast = null,
    )

    private fun day(label: String, hours: String, fraction: Float, writing: Float, fitness: Float, today: Boolean = false) = ReviewDay(
        label = label,
        hours = hours,
        isToday = today,
        fraction = fraction,
        segments = listOf(ReviewSegment("q1", BLUE, writing), ReviewSegment("q2", GREEN, fitness)).filter { it.fraction > 0f },
        a11yLabel = "$label: $hours",
    )

    val review = ReviewData(
        weekStart = "2026-11-23",
        weekLabel = "Nov 23 – 29",
        previousWeek = "2026-11-16",
        nextWeek = "2026-11-30",
        isThisWeek = true,
        questHours = "6.2h",
        target = "/ 20h",
        targetNote = "31% of your weekly target",
        targetProgress = 0.31f,
        dailyAverage = "1.2h",
        questShare = "78%",
        days = listOf(
            day("Mon", "2.5h", 1f, 0.7f, 0.3f),
            day("Tue", "1.5h", 0.6f, 0.6f, 0f),
            day("Wed", "2.2h", 0.88f, 0.5f, 0.38f, today = true),
            day("Thu", "0h", 0f, 0f, 0f),
            day("Fri", "0h", 0f, 0f, 0f),
            day("Sat", "0h", 0f, 0f, 0f),
            day("Sun", "0h", 0f, 0f, 0f),
        ),
        byQuest = listOf(ReviewQuest("q1", "Write the book", BLUE, "4.5h", 1f, "Write the book: 4.5h"), ReviewQuest("q2", "Fitness", GREEN, "1.7h", 0.38f, "Fitness: 1.7h")),
        byQuestA11y = "Hours by quest: Write the book 4.5h, Fitness 1.7h",
        questWork = "6.2h quest work",
        other = "1.8h other",
        questFraction = 0.78f,
        splitA11y = "6.2 hours quest work, 1.8 hours other",
        weeks = listOf(5f, 8f, 12f, 9f, 14f, 11f, 16f, 6.2f).mapIndexed { index, hours ->
            ReviewWeek(weekStart = "2026-${if (index < 4) "10" else "11"}-${10 + index}", label = "W${41 + index}", hours = "${hours}h", fraction = hours / 16f, selected = index == 7, a11yLabel = "Week ${41 + index}: ${hours}h")
        },
        weeksA11y = "Hours in the last 8 weeks",
    )

    val lastWeek = review.copy(weekStart = "2026-11-16", weekLabel = "Nov 16 – 22", previousWeek = "2026-11-09", nextWeek = "2026-11-23", isThisWeek = false)

    val archivedQuest = ArchiveItem(TrashKind.QUEST, "q8", "Old blog", "Archived quest", "#d6b64a")

    val archive = ArchiveData(
        archivedQuests = listOf(archivedQuest),
        groups = listOf(
            ArchiveGroup("Quests", listOf(ArchiveItem(TrashKind.QUEST, "q9", "Old hobby", "Deleted Nov 20 · 3 tasks", "#d66fa8"))),
            ArchiveGroup("Tasks", listOf(ArchiveItem(TrashKind.TASK, "t9", "Call the bank", "Deleted today", null))),
        ),
    )

    val emptyArchive = ArchiveData(emptyList(), emptyList())

    val commands = listOf(
        SearchResult("Go to Quests", "Jump", SearchKind.JUMP, SearchToken.Go(AppView.QUESTS)),
        SearchResult("Write the book", "Quest", SearchKind.QUEST, SearchToken.ShowQuest("q1")),
        SearchResult("Outline chapter 3", "Focus", SearchKind.TASK, SearchToken.Focus("t1")),
        SearchResult("Toggle theme", "Action", SearchKind.ACTION, SearchToken.ToggleTheme),
        SearchResult("Log work", "Action", SearchKind.ACTION, SearchToken.LogWork),
    )
}
