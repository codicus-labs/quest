package app.quest.android.ui.quests

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.failureMessage
import java.io.Serializable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import uniffi.quest_mobile.HueOption
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.QuestDetail
import uniffi.quest_mobile.QuestHue

/** What the editor edits: a new quest (nothing stored until it has a title) or a stored one. */
sealed interface EditorTarget : Serializable {
    /** [hue]: the colour picked for it; null keeps the core's choice. */
    data class New(val hue: QuestHue? = null) : EditorTarget

    data class Quest(val questId: String) : EditorTarget
}

/** The editor's drafts; in the SavedStateHandle, so the sheet and its typing survive process death. */
data class QuestEditorInputs(
    /** null: the sheet is closed. */
    val target: EditorTarget? = null,
    /** The title as typed; null shows the stored one. */
    val title: String? = null,
    /** Why Create was refused (an empty title); cleared by typing. */
    val createError: String? = null,
) : Serializable

sealed interface QuestEditorState {
    data object Closed : QuestEditorState

    /** "New quest": [hues] with the colour it will get selected. */
    data class New(val title: String, val hues: List<HueOption>, val error: String?) : QuestEditorState

    /** "Edit quest": the stored quest and the drafts over it. */
    data class Editing(val detail: QuestDetail, val title: String) : QuestEditorState
}

private const val INPUTS_KEY = "quest.editor"

/**
 * The quest editor over the Quests tab (the desktop's quest dialog): title and colour,
 * Complete/Reopen quest and Delete (each with Undo), and the way to the quest's page. "New quest"
 * opens it empty; the quest exists once Create takes its title. The title saves like the task
 * sheet's: on Done, on losing focus and on closing.
 */
class QuestEditorViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private val inputs: StateFlow<QuestEditorInputs> = savedState.getStateFlow(INPUTS_KEY, QuestEditorInputs())

    // Before [state]: reading it may report a failure.
    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    /** The quest as last read, so a failed re-read keeps the sheet (and its drafts) open. */
    private var lastDetail: QuestDetail? = null

    /** The core no longer knows the edited quest (deleted here, elsewhere or by sync). */
    private var questGone = false

    val state: StateFlow<QuestEditorState> = combine(core.state, inputs, ::editorState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, editorState(core.state.value, inputs.value))

    init {
        // A quest deleted elsewhere (or by sync) closes its editor.
        viewModelScope.launch {
            state.collect { if (it == QuestEditorState.Closed && inputs.value.target is EditorTarget.Quest && questGone) clear() }
        }
    }

    private fun update(change: (QuestEditorInputs) -> QuestEditorInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    private val editedQuest get() = (inputs.value.target as? EditorTarget.Quest)?.questId

    fun open(questId: String) = update { QuestEditorInputs(target = EditorTarget.Quest(questId)) }

    fun openNew() = update { QuestEditorInputs(target = EditorTarget.New()) }

    /** Saves the drafts of a stored quest and closes; a new quest without Create is dropped. */
    fun close() {
        commitDrafts()
        clear()
    }

    private fun clear() = update { QuestEditorInputs() }

    fun editTitle(text: String) = update { it.copy(title = text.replace('\n', ' '), createError = null) }

    /** The title field's Done: creates a new quest, or saves a stored one's title. */
    fun titleDone() = if (inputs.value.target is EditorTarget.New) create() else commitTitle()

    /** The core keeps the old title for an empty one (desktop rule). */
    fun commitTitle() {
        val id = editedQuest ?: return
        val title = inputs.value.title ?: return
        update { it.copy(title = null) }
        report(core.perform { c, _ -> c.renameQuest(id, title) })
    }

    private fun commitDrafts() = commitTitle()

    fun setHue(hue: QuestHue) {
        when (inputs.value.target) {
            is EditorTarget.New -> update { it.copy(target = EditorTarget.New(hue)) }
            is EditorTarget.Quest -> editedQuest?.let { id -> report(core.perform { c, _ -> c.setQuestHue(id, hue) }) }
            null -> Unit
        }
    }

    /** Create quest: with a title the quest exists and the sheet edits it; else the core's reason shows. */
    fun create() {
        val current = inputs.value
        val target = current.target as? EditorTarget.New ?: return
        val outcome = core.perform { c, _ -> c.createQuest(current.title.orEmpty(), target.hue) }
        when (outcome) {
            is Outcome.Done -> update { QuestEditorInputs(target = EditorTarget.Quest(outcome.value)) }
            else -> update { it.copy(createError = outcome.failureMessage()) }
        }
    }

    /** Complete quest / Reopen quest: the quest changes tab, the sheet closes, the snackbar offers Undo. */
    fun toggleStatus() {
        val id = editedQuest ?: return
        commitDrafts()
        report(core.perform { c, now -> c.toggleQuestStatus(id, now) })
        clear()
    }

    /** Delete: saves the drafts (as the desktop), the quest goes to the archive, the snackbar offers Undo. */
    fun delete() {
        val id = editedQuest ?: return
        commitDrafts()
        report(core.perform { c, now -> c.deleteQuest(id, now) })
        clear()
    }

    /** "Open quest page": saves and closes; returns the quest to open. */
    fun openPage(): String? {
        val id = editedQuest ?: return null
        close()
        return id
    }

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }

    private fun editorState(state: CoreState, inputs: QuestEditorInputs): QuestEditorState = when (val target = inputs.target) {
        null -> QuestEditorState.Closed
        is EditorTarget.New -> {
            val hues = (state as? CoreState.Ready)?.quests?.newQuestHues.orEmpty()
            val picked = target.hue?.let { hue -> hues.map { it.copy(selected = it.hue == hue) } } ?: hues
            QuestEditorState.New(inputs.title.orEmpty(), picked, inputs.createError)
        }
        is EditorTarget.Quest -> readDetail(target.questId)
            ?.let { QuestEditorState.Editing(it, inputs.title ?: it.title) }
            ?: QuestEditorState.Closed
    }

    /** The quest; null only when the core no longer knows it (deleted), not when a read fails. */
    private fun readDetail(questId: String): QuestDetail? {
        val outcome = core.read { c, now ->
            try {
                c.questDetail(questId, now)
            } catch (_: MobileException.UnknownQuest) {
                null
            }
        }
        questGone = outcome is Outcome.Done && outcome.value == null
        return when (outcome) {
            is Outcome.Done -> outcome.value.also { lastDetail = it }
            is Outcome.Failed -> {
                _message.value = outcome.message
                lastDetail?.takeIf { it.id == questId }
            }
            Outcome.NotReady -> lastDetail?.takeIf { it.id == questId }
        }
    }
}
