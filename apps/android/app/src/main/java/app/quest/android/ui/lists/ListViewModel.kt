package app.quest.android.ui.lists

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.ListPage
import app.quest.android.core.Outcome
import app.quest.android.core.failureMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.ListKind
import uniffi.quest_mobile.TaskList

sealed interface ListUiState {
    data object Loading : ListUiState

    data class Ready(val list: TaskList, val draft: String) : ListUiState

    /** The quest whose list this was is gone (deleted here or elsewhere). */
    data object Gone : ListUiState

    data class Failed(val message: String) : ListUiState
}

private const val DRAFT_KEY = "list.draft"

/**
 * The shown list (Today, Inbox, Logbook or a quest's): the core reads it only while [show] holds a kind.
 * The quick-add draft survives process death.
 */
class ListViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private val shown = MutableStateFlow<ListKind?>(null)
    private val draft: StateFlow<String> = savedState.getStateFlow(DRAFT_KEY, "")

    val state: StateFlow<ListUiState> = combine(core.state, shown, draft, ::listState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, ListUiState.Loading)

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    /** The last list shown, so going back to it keeps its draft. */
    private var lastShown: ListKind? = null

    /** Shows [kind] (null: no list on screen). Another list than the last starts with an empty draft. */
    fun show(kind: ListKind?) {
        if (kind != null) {
            if (lastShown != null && kind != lastShown) savedState[DRAFT_KEY] = ""
            lastShown = kind
        }
        shown.value = kind
        core.setListShown(kind)
    }

    fun editDraft(text: String) {
        savedState[DRAFT_KEY] = text.replace('\n', ' ')
    }

    /** Adds the draft to the shown list with its defaults (Today: planned today, a quest's list: that quest). */
    fun add() {
        val kind = shown.value ?: return
        val title = draft.value
        if (title.isBlank()) return
        val outcome = core.perform { c, now -> c.addToList(kind, title, now) }
        report(outcome)
        if (outcome is Outcome.Done) savedState[DRAFT_KEY] = ""
    }

    fun toggleTask(taskId: String) = report(core.perform { c, now -> c.toggleTask(taskId, now) })

    /** Starts [taskId]'s timer (stopping another); true when it started. */
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

private fun listState(state: CoreState, shown: ListKind?, draft: String): ListUiState = when (state) {
    CoreState.Opening -> ListUiState.Loading
    is CoreState.Failed -> ListUiState.Failed(state.message)
    is CoreState.Ready -> when (val page = state.list?.takeIf { it.kind == shown }) {
        null -> ListUiState.Loading
        is ListPage.Gone -> ListUiState.Gone
        is ListPage.Shown -> ListUiState.Ready(page.list, draft)
    }
}
