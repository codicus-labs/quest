package app.quest.android.ui.quests

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.QuestPage
import app.quest.android.core.failureMessage
import app.quest.android.settings.HomePreferences
import app.quest.android.settings.HomePrefs
import java.io.Serializable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import uniffi.quest_mobile.QuestDetail
import uniffi.quest_mobile.QuestHue

/** The quest page's drafts; in the SavedStateHandle, so the page and its typing survive process death. */
data class QuestDetailInputs(
    /** The open quest; null: the page is closed. */
    val questId: String? = null,
    /** The title being renamed in place; null shows the stored one. */
    val title: String? = null,
    val taskDraft: String = "",
    /** The add field adds to the backlog instead of today. */
    val toBacklog: Boolean = false,
) : Serializable

sealed interface QuestDetailUiState {
    /** No quest page open. */
    data object Hidden : QuestDetailUiState

    data object Loading : QuestDetailUiState

    data class Ready(val detail: QuestDetail, val inputs: QuestDetailInputs) : QuestDetailUiState {
        val title get() = inputs.title ?: detail.title

        /** The name is a field in the heading's place while a new one is typed. */
        val renamingTitle get() = inputs.title != null
    }

    data class Failed(val message: String) : QuestDetailUiState
}

private const val INPUTS_KEY = "quest.detail"

/**
 * One quest's page over the tabs: the working view of its tasks (as on the desktop; description
 * and the rest are in the editor). The core reads the quest only while the page is open; a quest
 * gone (deleted here or elsewhere) closes it. The title saves on Done, on losing focus and on leaving.
 */
class QuestDetailViewModel(
    private val core: CoreHolder,
    private val savedState: SavedStateHandle,
    preferences: HomePreferences,
) : ViewModel() {
    private val inputs: StateFlow<QuestDetailInputs> = savedState.getStateFlow(INPUTS_KEY, QuestDetailInputs())
    private val prefs: StateFlow<HomePrefs?> = preferences.prefs.stateIn(viewModelScope, SharingStarted.Eagerly, null)

    val state: StateFlow<QuestDetailUiState> = combine(core.state, inputs, ::detailState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, detailState(core.state.value, inputs.value))

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    init {
        core.setQuestPage(inputs.value.questId)
        viewModelScope.launch {
            core.state.collect { state ->
                val page = (state as? CoreState.Ready)?.questPage
                if (page is QuestPage.Gone && page.questId == inputs.value.questId) close(save = false)
            }
        }
    }

    private fun update(change: (QuestDetailInputs) -> QuestDetailInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    private val questId get() = inputs.value.questId

    /** Opens [questId]'s page; another open page saves its drafts first. */
    fun open(questId: String) {
        if (questId == this.questId) return
        commitDrafts()
        savedState[INPUTS_KEY] = QuestDetailInputs(questId = questId)
        core.setQuestPage(questId)
    }

    /** Leaves the page, saving the title unless the quest is gone. */
    fun close(save: Boolean = true) {
        if (save) commitDrafts()
        savedState[INPUTS_KEY] = QuestDetailInputs()
        core.setQuestPage(null)
    }

    // ---- Title, colour, status ----

    /** Turns the name into a field holding the stored name. */
    fun beginRename() {
        val ready = state.value as? QuestDetailUiState.Ready ?: return
        if (ready.renamingTitle) return
        update { it.copy(title = ready.detail.title) }
    }

    fun editTitle(text: String) = update { it.copy(title = text.replace('\n', ' ')) }

    /** The core keeps the old title for an empty one (desktop rule). */
    fun commitTitle() {
        val id = questId ?: return
        val title = inputs.value.title ?: return
        update { it.copy(title = null) }
        report(core.perform { c, _ -> c.renameQuest(id, title) })
    }

    fun commitDrafts() = commitTitle()

    fun setHue(hue: QuestHue) = questId?.let { id -> report(core.perform { c, _ -> c.setQuestHue(id, hue) }) }

    /** Complete quest, Reopen quest or Unarchive (the core decides by the status); the snackbar offers Undo. */
    fun toggleStatus() = questId?.let { id -> report(core.perform { c, now -> c.toggleQuestStatus(id, now) }) }

    /** Deletes the quest (the toast where the user lands offers Undo) and closes the page. */
    fun delete() {
        val id = questId ?: return
        val outcome = core.perform { c, now -> c.deleteQuest(id, now) }
        report(outcome)
        if (outcome is Outcome.Done) close(save = false)
    }

    // ---- Tasks ----

    fun editTaskDraft(text: String) = update { it.copy(taskDraft = text.replace('\n', ' ')) }

    fun setToBacklog(backlog: Boolean) = update { it.copy(toBacklog = backlog) }

    /** Adds the draft to the quest, today or in the backlog, at Home's add position. */
    fun addTask() {
        val id = questId ?: return
        val current = inputs.value
        if (current.taskDraft.isBlank()) return
        // Before the preferences have loaded, Home's default position.
        val position = (prefs.value ?: HomePrefs()).addPosition
        val outcome = core.perform { c, now -> c.addQuestTask(id, current.taskDraft, current.toBacklog, position, now) }
        report(outcome)
        if (outcome is Outcome.Done) update { it.copy(taskDraft = "") }
    }

    fun toggleTask(taskId: String) = report(core.perform { c, now -> c.toggleTask(taskId, now) })

    fun deleteTask(taskId: String) = report(core.perform { c, now -> c.deleteTask(taskId, now) })

    /** Starts [taskId]'s timer; true when it started. */
    fun startTimer(taskId: String): Boolean {
        val outcome = core.perform { c, now -> c.startTimer(taskId, now) }
        report(outcome)
        return outcome is Outcome.Done
    }

    fun stopTimer() = report(core.perform { c, now -> c.stopTimer(now) })

    fun undo() = report(core.perform { c, now -> c.undo(now) })

    fun dismissToast() = report(core.perform { c, now -> c.dismissToast(now) })

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }
}

private fun detailState(state: CoreState, inputs: QuestDetailInputs): QuestDetailUiState {
    val questId = inputs.questId ?: return QuestDetailUiState.Hidden
    return when (state) {
        CoreState.Opening -> QuestDetailUiState.Loading
        is CoreState.Failed -> QuestDetailUiState.Failed(state.message)
        is CoreState.Ready -> when (val page = state.questPage) {
            is QuestPage.Shown -> if (page.questId == questId) QuestDetailUiState.Ready(page.detail, inputs) else QuestDetailUiState.Loading
            // Gone: the collector closes the page.
            is QuestPage.Gone, null -> QuestDetailUiState.Loading
        }
    }
}
