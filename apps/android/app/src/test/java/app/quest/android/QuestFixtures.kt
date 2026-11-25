package app.quest.android

import uniffi.quest_mobile.AppView
import uniffi.quest_mobile.ArchiveData
import uniffi.quest_mobile.ArchiveGroup
import uniffi.quest_mobile.ArchiveItem
import uniffi.quest_mobile.HomeTask
import uniffi.quest_mobile.HueOption
import uniffi.quest_mobile.QuestCard
import uniffi.quest_mobile.QuestDetail
import uniffi.quest_mobile.QuestHue
import uniffi.quest_mobile.QuestTaskGroup
import uniffi.quest_mobile.QuestsData
import uniffi.quest_mobile.ReviewBucket
import uniffi.quest_mobile.ReviewMetrics
import uniffi.quest_mobile.ReviewPostponed
import uniffi.quest_mobile.ReviewShare
import uniffi.quest_mobile.ReviewSince
import uniffi.quest_mobile.WeeklyReviewData
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
        HomeTask(id = id, title = title, done = done, quest = Samples.writing, tracked = null, timerRunning = false, canStartTimer = !done, plannedOn = null, dueOn = null, overdue = false, postponed = Samples.notPostponed)

    val detail = QuestDetail(
        id = "q1",
        title = "Write the book",
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
        toast = null,
    )

    private const val HOUR_MS = 3_600_000L

    val review = WeeklyReviewData(
        weekStart = "2026-11-23",
        weekLabel = "Nov 23 – 29",
        current = ReviewMetrics(4u, 8u, 7u, 12 * HOUR_MS + 2_880_000),
        previous = ReviewMetrics(2u, 3u, 3u, 8 * HOUR_MS + 2_160_000),
        goalMs = 20 * HOUR_MS,
        mostPostponed = listOf(
            ReviewPostponed("t1", "Outline chapter 3", false, 4u,
                listOf(listOf("2026-11-20", "2026-11-21", "2026-11-22", "2026-11-23")),
                listOf(listOf("2026-11-27", "2026-11-29"))),
            ReviewPostponed("t2", "Draft this week’s essay", false, 2u,
                listOf(listOf("2026-11-24", "2026-11-27")),
                listOf(listOf("2026-11-26", "2026-11-29"))),
            ReviewPostponed("t3", "Write the launch announcement", true, 1u,
                emptyList(), listOf(listOf("2026-11-24", "2026-12-02"))),
        ),
        carriedOver = listOf(ReviewSince("t4", "Review the interview notes", "2026-11-24")),
        overdue = listOf(ReviewSince("t5", "Send the course registration", "2026-11-26")),
        trackedMsPerDay = listOf(4 * HOUR_MS, 3 * HOUR_MS, 2 * HOUR_MS, 3 * HOUR_MS, 3 * HOUR_MS, 0L, 0L),
        shares = listOf(
            ReviewShare(ReviewBucket.Quest("q1", "Write the book", BLUE), 9 * HOUR_MS, 0.60),
            ReviewShare(ReviewBucket.Quest("q2", "Fitness", GREEN), 3 * HOUR_MS + 2_880_000, 0.2533),
            ReviewShare(ReviewBucket.Organizing, HOUR_MS + 1_800_000, 0.10),
            ReviewShare(ReviewBucket.NoQuest, 2_520_000, 0.0467),
        ),
    )

    val lastWeek = review.copy(
        weekStart = "2026-11-16",
        weekLabel = "Nov 16 – 22",
        current = review.previous,
        previous = ReviewMetrics(3u, 5u, 0u, 5 * HOUR_MS),
        mostPostponed = listOf(
            ReviewPostponed("t4", "Review the interview notes", false, 3u,
                listOf(listOf("2026-11-16", "2026-11-18", "2026-11-19", "2026-11-24")), emptyList()),
        ),
        carriedOver = emptyList(),
        overdue = emptyList(),
        trackedMsPerDay = listOf(2 * HOUR_MS, HOUR_MS, 0L, 3 * HOUR_MS, 0L, 2 * HOUR_MS + 2_160_000, 0L),
        shares = listOf(review.shares.first().copy(trackedMs = review.previous.questMs, fraction = 1.0)) +
            review.shares.takeLast(2).map { it.copy(trackedMs = 0, fraction = 0.0) },
    )

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
