package app.quest.android.ui.calendar

import java.io.Serializable
import uniffi.quest_mobile.EntryCheck
import uniffi.quest_mobile.EntryDraft
import uniffi.quest_mobile.LogWorkCheck
import uniffi.quest_mobile.LogWorkDraft
import uniffi.quest_mobile.TaskChoices

/** The Log work sheet's fields ([LogWorkDraft]), kept in the SavedStateHandle. */
data class LogWorkForm(
    val taskId: String?,
    val newTitle: String,
    val questId: String?,
    val day: String,
    val startMinute: Int,
    val endMinute: Int,
) : Serializable {
    fun toDraft() = LogWorkDraft(taskId, newTitle, questId, day, startMinute.toUInt(), endMinute.toUInt())

    companion object {
        fun of(draft: LogWorkDraft) =
            LogWorkForm(draft.taskId, draft.newTitle, draft.questId, draft.day, draft.startMinute.toInt(), draft.endMinute.toInt())
    }
}

/**
 * The entry editor's fields ([EntryDraft]), kept in the SavedStateHandle. [running]: the timer ran
 * when the draft was read, so its end fields mean nothing yet.
 */
data class EntryForm(
    val entryId: String,
    val taskId: String?,
    val startDay: String,
    val startMinute: Int,
    val endDay: String,
    val endMinute: Int,
    val endsOtherDay: Boolean,
    val running: Boolean = false,
) : Serializable {
    fun toDraft() = EntryDraft(entryId, taskId, startDay, startMinute.toUInt(), endDay, endMinute.toUInt(), endsOtherDay)

    companion object {
        fun of(draft: EntryDraft, running: Boolean = false) = EntryForm(
            draft.entryId,
            draft.taskId,
            draft.startDay,
            draft.startMinute.toInt(),
            draft.endDay,
            draft.endMinute.toInt(),
            draft.endsOtherDay,
            running,
        )
    }
}

/** The calendar's open sheet: none, Log work, or the editor of one entry. */
sealed interface CalendarSheet : Serializable {
    data object None : CalendarSheet {
        private fun readResolve(): Any = None
    }

    data class LogWork(val form: LogWorkForm) : CalendarSheet

    data class Entry(val form: EntryForm) : CalendarSheet
}

/**
 * What the user picked or typed on the calendar; in the SavedStateHandle, so it survives rotation
 * and process death. [taskQuery] is the task search while [pickingTask] (a page of the open sheet).
 * [saveError] is the core's reason a save was refused, until the form changes.
 */
data class CalendarInputs(
    /** The page's first day (ISO); null: the page with today. */
    val firstDay: String? = null,
    val sheet: CalendarSheet = CalendarSheet.None,
    val pickingTask: Boolean = false,
    val taskQuery: String = "",
    val saveError: String? = null,
) : Serializable

/** The Log work sheet: its fields and the core's live check of them. */
data class LogWorkState(val form: LogWorkForm, val check: LogWorkCheck, val saveError: String?)

/** The entry editor: its fields and the core's live check of them. */
data class EntryState(val form: EntryForm, val check: EntryCheck, val saveError: String?)

/** The task search page of a sheet. [canCreate]: Log work offers a new, completed task. */
data class TaskPickerState(val query: String, val choices: TaskChoices, val canCreate: Boolean)
