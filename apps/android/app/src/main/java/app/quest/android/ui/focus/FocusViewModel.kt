package app.quest.android.ui.focus

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.ui.task.ChecklistActions
import app.quest.android.ui.task.NoteActions
import app.quest.android.ui.task.parseNote
import app.quest.android.ui.task.toggleNoteMarker
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
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
import uniffi.quest_mobile.FocusPickTask
import uniffi.quest_mobile.ChecklistRow
import uniffi.quest_mobile.FocusSession
import uniffi.quest_mobile.UndoToast
import uniffi.quest_mobile.FocusView
import uniffi.quest_mobile.QuestChip

/**
 * Focus's inputs, in the [SavedStateHandle] so they survive process death: whether Focus is open,
 * the task started or picked last on this device (the core's `focus_task`), and the title and
 * description being typed for [draftTask].
 */
data class FocusInputs(
    val visible: Boolean = false,
    val focusTask: String? = null,
    val draftTask: String? = null,
    val titleDraft: String? = null,
    val descriptionDraft: String? = null,
) : Serializable {
    fun withoutDrafts() = copy(draftTask = null, titleDraft = null, descriptionDraft = null)
}

sealed interface FocusUiState {
    /** Focus is not open; nothing is read. */
    data object Hidden : FocusUiState

    data object Loading : FocusUiState

    data class Failed(val message: String) : FocusUiState

    data class Session(
        val session: FocusSession,
        /** The title as typed, else the task's. */
        val title: String,
        val description: String,
        /** The quests to pick from for the chip. */
        val quests: List<QuestChip>,
        val checklist: List<ChecklistRow> = emptyList(),
        val toast: UndoToast? = null,
    ) : FocusUiState

    /** Nothing to focus on: "What are you working on?". */
    data class Pick(val tasks: List<FocusPickTask>) : FocusUiState
}

private const val INPUTS_KEY = "focus.inputs"

/** The Focus screen (desktop `ui/focus.rs`): one task with its timer, or a list to start one. */
class FocusViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private val inputs: StateFlow<FocusInputs> = savedState.getStateFlow(INPUTS_KEY, FocusInputs())

    private var noteSave: Job? = null

    /** Re-read on every core change and tick, so the timer counts every second while it runs. */
    val state: StateFlow<FocusUiState> = combine(core.state, inputs, ::focusState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, focusState(core.state.value, inputs.value))

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    private fun focusState(coreState: CoreState, inputs: FocusInputs): FocusUiState {
        if (!inputs.visible) return FocusUiState.Hidden
        val ready = when (coreState) {
            CoreState.Opening -> return FocusUiState.Loading
            is CoreState.Failed -> return FocusUiState.Failed(coreState.message)
            is CoreState.Ready -> coreState
        }
        val read = core.read { c, now ->
            val view = c.focus(now, inputs.focusTask)
            view to if (view is FocusView.Session) c.checklist(view.session.taskId) else emptyList()
        }
        return when (read) {
            is Outcome.Done -> when (val view = read.value.first) {
                is FocusView.Pick -> FocusUiState.Pick(view.tasks)
                is FocusView.Session -> {
                    val session = view.session
                    val drafts = inputs.takeIf { it.draftTask == session.taskId }
                    FocusUiState.Session(
                        session = session,
                        title = drafts?.titleDraft ?: session.title,
                        description = drafts?.descriptionDraft ?: session.description,
                        quests = ready.home.quests,
                        checklist = read.value.second,
                        toast = ready.home.toast,
                    )
                }
            }
            is Outcome.Failed -> FocusUiState.Failed(read.message)
            Outcome.NotReady -> FocusUiState.Loading
        }
    }

    private fun update(change: (FocusInputs) -> FocusInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    private fun sessionTask(): String? = (state.value as? FocusUiState.Session)?.session?.taskId

    /** Opens Focus; with [taskId], on that task (it becomes the device's focus task). */
    fun open(taskId: String? = null) = update { it.copy(visible = true, focusTask = taskId ?: it.focusTask) }

    /**
     * "Start timer on last task" (launcher shortcut, maybe a cold start): starts the timer on the
     * core's focus target unless it already runs, then opens Focus; with nothing to start, Focus's pick.
     * Calls [onStarted] when a timer started.
     */
    fun startLastTask(onStarted: () -> Unit = {}) {
        viewModelScope.launch {
            if (!core.awaitOpen()) return@launch
            // null like the shortcut's label (AppShortcuts): the core's last task, so the label and the start agree.
            val view = core.read { c, now -> c.focus(now, null) }
            val session = ((view as? Outcome.Done)?.value as? FocusView.Session)?.session
            if (session != null && !session.running && start(session.taskId)) onStarted()
            open()
        }
    }

    /** A timer started elsewhere (a card): Focus shows that task from now on. */
    fun rememberTask(taskId: String) = update { it.copy(focusTask = taskId) }

    /** Back to Home; typed title and description are saved first. */
    fun close() {
        commitDrafts()
        update { it.copy(visible = false) }
    }

    fun pause() = report(core.perform { c, now -> c.stopTimer(now) })

    /** True if the timer started. */
    fun resume(): Boolean {
        val taskId = sessionTask() ?: return false
        return start(taskId)
    }

    /** "What are you working on?": starts the timer on [taskId]; true if it started. */
    fun pick(taskId: String): Boolean = start(taskId)

    private fun start(taskId: String): Boolean {
        val outcome = core.perform { c, now -> c.startTimer(taskId, now) }
        report(outcome)
        if (outcome is Outcome.Done) rememberTask(taskId)
        return outcome is Outcome.Done
    }

    /** Completes the task (stops its timer; Home shows the undo toast) and goes back to Home. */
    fun complete() {
        val taskId = sessionTask() ?: return
        commitDrafts()
        val outcome = core.perform { c, now -> c.completeTask(taskId, now) }
        report(outcome)
        if (outcome is Outcome.Done) update { it.copy(visible = false) }
    }

    /** null: "No quest". */
    fun setQuest(questId: String?) {
        val taskId = sessionTask() ?: return
        report(core.perform { c, _ -> c.setTaskQuest(taskId, questId) })
    }

    fun editTitle(text: String) = editDraft { it.copy(titleDraft = text.replace('\n', ' ')) }

    fun editDescription(text: String) {
        editDraft { it.copy(descriptionDraft = text) }
        noteSave?.cancel()
        noteSave = viewModelScope.launch {
            delay(300)
            commitDescription()
        }
    }

    val notes = NoteActions(::editDescription, ::commitDescription, core::parseNote) { source, offset ->
        sessionTask()?.let { report(core.toggleNoteMarker(it, source, offset)) }
    }

    val checklist = ChecklistActions(
        toggle = { id, done -> report(core.perform { c, now -> c.setChecklistItemDoneWithUndo(id, done, now) }) },
    )

    fun undo() = report(core.perform { c, now -> c.undo(now) })

    fun dismissToast() = report(core.perform { c, now -> c.dismissToast(now) })

    /** Drafts belong to the task shown when typing started; switching tasks starts fresh ones. */
    private fun editDraft(change: (FocusInputs) -> FocusInputs) {
        val taskId = sessionTask() ?: return
        update { current ->
            val base = if (current.draftTask == taskId) current else current.withoutDrafts().copy(draftTask = taskId)
            change(base)
        }
    }

    /** Saves the typed title; the core ignores an empty or unchanged one (desktop rule). */
    fun commitTitle() {
        val current = inputs.value
        val taskId = current.draftTask ?: return
        val title = current.titleDraft ?: return
        update { it.copy(titleDraft = null) }
        report(core.perform { c, _ -> c.renameTask(taskId, title) })
    }

    fun commitDescription() {
        noteSave?.cancel()
        noteSave = null
        val current = inputs.value
        val taskId = current.draftTask ?: return
        val description = current.descriptionDraft ?: return
        update { it.copy(descriptionDraft = null) }
        report(core.perform { c, _ -> c.setTaskDescription(taskId, description) })
    }

    /** Another workspace is shown: typed text is saved to its task first, then Focus forgets the task. */
    fun leaveWorkspace() {
        commitDrafts()
        update { it.copy(focusTask = null) }
    }

    /** Saves what is typed (also when the app goes to the background, which may end the process). */
    fun commitDrafts() {
        commitTitle()
        commitDescription()
        update { it.withoutDrafts() }
    }

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }
}
