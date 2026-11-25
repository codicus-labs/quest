package app.quest.android

import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.FormError
import uniffi.quest_mobile.FormField
import uniffi.quest_mobile.LogWorkCheck
import uniffi.quest_mobile.LogWorkDraft
import uniffi.quest_mobile.TaskChoices
import uniffi.quest_mobile.TaskOption

/** The Log work sheet's samples on Monday 2026-11-23 (today, 09:30). */
object LogWorkSamples {
    val draft = LogWorkDraft(taskId = null, newTitle = "", questId = null, day = "2026-11-23", startMinute = 9u * 60u + 15u, endMinute = 9u * 60u + 30u)

    private val today = DayValue(iso = "2026-11-23", label = "Nov 23, 2026")

    val check = LogWorkCheck(
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
    val existingTask = check.copy(taskTitle = "Morning run", quest = Samples.fitness, error = null)

    /** Log work for a new task with a quest chosen; the end is before the start. */
    val newTaskError = check.copy(
        taskTitle = "Plan the workshop",
        isNewTask = true,
        quest = Samples.writing,
        canChooseQuest = true,
        startLabel = "14:00",
        endLabel = "13:30",
        duration = "–",
        error = FormError(FormField.END, "The end must be after the start."),
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
