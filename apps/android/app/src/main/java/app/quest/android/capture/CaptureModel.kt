package app.quest.android.capture

import app.quest.android.ui.task.RepeatForm
import java.io.Serializable

/*
 * The AI capture's draft and view as the app holds them: the core's UniFFI records (`CaptureDraft`,
 * `CaptureView`, …), mirrored so a draft can sit in a `SavedStateHandle` (java.io.Serializable, as the other
 * forms) and so screens and tests do not depend on the generated types. `RustQuestCore` translates; the
 * core alone decides what a draft means (labels, errors, what Create makes).
 */

/** A quest the draft points at: one that exists, or one of the draft's new quests (by its key). */
sealed interface DraftQuestRef : Serializable {
    data class Existing(val id: String) : DraftQuestRef

    data class New(val key: String) : DraftQuestRef
}

/** A day as the core labels it ("2026-11-25", "Tomorrow"). */
data class DraftDay(val iso: String, val label: String) : Serializable

data class DraftQuest(val key: String, val selected: Boolean, val title: String) : Serializable

data class DraftTask(
    val key: String,
    val selected: Boolean,
    val title: String,
    val description: String,
    val quest: DraftQuestRef?,
    /** The model was unsure about [quest]; shown until the user picks one. */
    val questUncertain: Boolean,
    /** The plan date; null: not planned. */
    val plannedOn: DraftDay?,
    /** The deadline; kept as proposed (the app does not edit it yet). */
    val dueOn: DraftDay?,
    val repeat: RepeatForm?,
) : Serializable

/** The task a time entry goes on: an existing one, or a new one with its quest. */
sealed interface DraftWorkTask : Serializable {
    data class Existing(val id: String, val title: String) : DraftWorkTask

    data class New(val title: String, val quest: DraftQuestRef?) : DraftWorkTask
}

data class DraftWork(
    val key: String,
    val selected: Boolean,
    val task: DraftWorkTask,
    /** The model was unsure which task this was. */
    val uncertain: Boolean,
    val day: DraftDay,
    /** Minutes after midnight as picked (0..1439); the core reads an end of 0 as midnight. */
    val startMinutes: Int,
    val endMinutes: Int,
) : Serializable

/** What the model proposed, as edited so far. [dropped] are the core's sentences about what it left out. */
data class CaptureDraft(
    val quests: List<DraftQuest>,
    val tasks: List<DraftTask>,
    val work: List<DraftWork>,
    val dropped: List<String>,
) : Serializable {
    fun withQuest(key: String, change: (DraftQuest) -> DraftQuest) = copy(quests = quests.map { if (it.key == key) change(it) else it })

    fun withTask(key: String, change: (DraftTask) -> DraftTask) = copy(tasks = tasks.map { if (it.key == key) change(it) else it })

    fun withWork(key: String, change: (DraftWork) -> DraftWork) = copy(work = work.map { if (it.key == key) change(it) else it })
}

/** A quest as a chip: name and colour ("#rrggbb"). */
data class CaptureChip(val name: String, val colorHex: String)

data class DraftQuestView(val key: String, val colorHex: String, val error: String?)

data class DraftTaskView(
    val key: String,
    val quest: CaptureChip?,
    /** "Tomorrow · due Fri, Nov 27", "Someday". */
    val whenLabel: String,
    val repeatSummary: String?,
    val error: String?,
)

data class DraftWorkView(
    val key: String,
    val taskTitle: String,
    val quest: CaptureChip?,
    /** "Today · 09:00–11:00". */
    val whenLabel: String,
    /** "2:00". */
    val duration: String,
    /** "09:00", "24:00". */
    val startLabel: String,
    val endLabel: String,
    val error: String?,
)

/** One entry of the quest picker (the picker lists "No quest" first itself). */
data class DraftQuestOption(val quest: DraftQuestRef, val chip: CaptureChip)

/** The core's reading of a draft: labels, errors, the picker's quests and the Create button. */
data class CaptureView(
    val quests: List<DraftQuestView>,
    val tasks: List<DraftTaskView>,
    val work: List<DraftWorkView>,
    val questOptions: List<DraftQuestOption>,
    val selectedCount: Int,
    val canApply: Boolean,
    /** "Create (3)". */
    val applyLabel: String,
)

/** Why capturing did not work; the sheet shows a hint for each. */
sealed interface CaptureFailure {
    data object SignedOut : CaptureFailure

    /** The server has no AI provider configured; the entries hide once the account reports it. */
    data object NotConfigured : CaptureFailure

    /** [message]: the core's, with what failed on the way to the server. */
    data class Offline(val message: String) : CaptureFailure

    data object Unavailable : CaptureFailure

    data object RateLimited : CaptureFailure

    data object InvalidResponse : CaptureFailure

    /** [message]: the core's, with the limit. */
    data class TextTooLong(val message: String) : CaptureFailure

    /** Anything else, with the core's message. */
    data class Other(val message: String) : CaptureFailure
}

/** A capture call failed for [failure]; thrown by `QuestCoreApi.capture`. */
class CaptureException(val failure: CaptureFailure) : Exception(failure.toString())

/** What Create did: [message] with an Undo action ([undoLabel]), shown for [showForMs]. */
data class CaptureApplied(val message: String, val undoLabel: String, val showForMs: Long)
