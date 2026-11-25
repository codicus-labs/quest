package app.quest.android

import app.quest.android.capture.CaptureChip
import app.quest.android.capture.CaptureDraft
import app.quest.android.capture.CaptureView
import app.quest.android.capture.DraftBlock
import app.quest.android.capture.DraftDay
import app.quest.android.capture.DraftMilestone
import app.quest.android.capture.DraftQuest
import app.quest.android.capture.DraftQuestOption
import app.quest.android.capture.DraftQuestRef
import app.quest.android.capture.DraftQuestView
import app.quest.android.capture.DraftTask
import app.quest.android.capture.DraftTaskView
import app.quest.android.capture.DraftWork
import app.quest.android.capture.DraftWorkTask
import app.quest.android.capture.DraftWorkView
import app.quest.android.ui.task.RepeatForm
import uniffi.quest_mobile.RepeatFrequency
import uniffi.quest_mobile.RepeatPinChoice

/** A proposal for "Tomorrow write the offer … New quest Tax return … Worked on the concept from 9 to 11." */
object CaptureSamples {
    private val relaunch = CaptureChip("Website relaunch", "#4f7cd9")
    private val tax = CaptureChip("Tax return", "#62b87a")

    val weekly = RepeatForm(
        every = "1", frequency = RepeatFrequency.WEEK, pin = RepeatPinChoice.NONE, weekdays = listOf(), lastDay = false,
        monthDay = "", starts = "2026-11-27", endsOnDate = false, ends = "", timeZone = "Europe/Berlin",
    )

    val draft = CaptureDraft(
        quests = listOf(
            DraftQuest("q1", true, "Tax return", "Everything for the 2025 return", listOf(DraftMilestone("Receipts collected", DraftDay("2026-11-27", "Fri, Nov 27")))),
        ),
        tasks = listOf(
            DraftTask(
                "t1", true, "Write offer for Miller", "", DraftQuestRef.Existing("q-relaunch"), questUncertain = false,
                day = DraftDay("2026-11-25", "Tomorrow"), block = DraftBlock(9 * 60, 11 * 60), repeat = null,
            ),
            DraftTask(
                "t2", true, "Collect receipts", "", DraftQuestRef.New("q1"), questUncertain = true,
                day = DraftDay("2026-11-27", "Fri, Nov 27"), block = null, repeat = weekly,
            ),
        ),
        work = listOf(
            DraftWork("w1", true, DraftWorkTask.New("Relaunch concept", DraftQuestRef.Existing("q-relaunch")), uncertain = true, DraftDay("2026-11-24", "Today"), 9 * 60, 11 * 60),
        ),
        dropped = listOf("“Pay rent on the 31st” was left out: November has no 31st."),
    )

    /** The core's reading, simplified: a deselected new quest leaves its tasks without a quest. */
    fun view(draft: CaptureDraft): CaptureView {
        val newQuests = draft.quests.filter { it.selected }.map { it.key }.toSet()
        fun chip(ref: DraftQuestRef?) = when (ref) {
            is DraftQuestRef.Existing -> relaunch
            is DraftQuestRef.New -> if (ref.key in newQuests) tax else null
            null -> null
        }
        val count = draft.quests.count { it.selected } + draft.tasks.count { it.selected } + draft.work.count { it.selected }
        val badTitle = draft.tasks.any { it.selected && it.title.isBlank() }
        return CaptureView(
            quests = draft.quests.map { DraftQuestView(it.key, "#62b87a", if (it.milestones.isEmpty()) "" else "${it.milestones.size} milestone", null) },
            tasks = draft.tasks.map { t ->
                DraftTaskView(
                    t.key, chip(t.quest),
                    whenLabel = listOfNotNull(t.day?.label ?: "Someday", t.block?.let { "09:00–11:00" }).joinToString(" · "),
                    blockStartLabel = t.block?.let { "09:00" }, blockEndLabel = t.block?.let { "11:00" },
                    repeatSummary = t.repeat?.let { "Every week on Fri" },
                    error = if (t.selected && t.title.isBlank()) "Enter a title." else null,
                )
            },
            work = draft.work.map { w ->
                val (title, quest) = when (val task = w.task) {
                    is DraftWorkTask.Existing -> task.title to null
                    is DraftWorkTask.New -> task.title to task.quest
                }
                DraftWorkView(w.key, title, chip(quest), "${w.day.label} · 09:00–11:00", "2:00", "09:00", "11:00", null)
            },
            questOptions = listOf(DraftQuestOption(DraftQuestRef.Existing("q-relaunch"), relaunch)) +
                draft.quests.filter { it.selected }.map { DraftQuestOption(DraftQuestRef.New(it.key), tax) },
            selectedCount = count,
            canApply = count > 0 && !badTitle,
            applyLabel = "Create ($count)",
        )
    }
}
