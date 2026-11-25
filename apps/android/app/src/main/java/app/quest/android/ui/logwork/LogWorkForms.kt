package app.quest.android.ui.logwork

import java.io.Serializable
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
 * What the user typed into Log work; in the SavedStateHandle, so it survives rotation and process
 * death. [form] is null while the sheet is closed. [taskQuery] is the task search while
 * [pickingTask] (a page of the sheet). [saveError] is the core's reason a save was refused, until
 * the form changes.
 */
data class LogWorkInputs(
    val form: LogWorkForm? = null,
    val pickingTask: Boolean = false,
    val taskQuery: String = "",
    val saveError: String? = null,
) : Serializable

/** The Log work sheet: its fields and the core's live check of them. */
data class LogWorkState(val form: LogWorkForm, val check: LogWorkCheck, val saveError: String?)

/** The task search page of the sheet. */
data class TaskPickerState(val query: String, val choices: TaskChoices)

/** The open sheet: its form, its task search (null while it shows its fields), or a failed check. */
data class LogWorkSheetState(val logWork: LogWorkState?, val picker: TaskPickerState?, val error: String? = null)
