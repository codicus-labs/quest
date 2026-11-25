package app.quest.android.ui.lists

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.ListPage
import app.quest.android.core.Outcome
import app.quest.android.core.failureMessage
import app.quest.android.core.readCalendar
import app.quest.android.core.shiftTaskMonth
import uniffi.quest_mobile.DateTarget
import uniffi.quest_mobile.QuickFields
import uniffi.quest_mobile.updateQuickDate
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

    data class Ready(val list: TaskList, val draft: String, val quick: QuickAddFields? = null) : ListUiState

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

    private val overrides = MutableStateFlow<QuickFields?>(null)

    val state: StateFlow<ListUiState> = combine(core.state, shown, draft, overrides, ::listState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, ListUiState.Loading)

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    /** The last list shown, so going back to it keeps its draft. */
    private var lastShown: ListKind? = null

    /** Shows [kind] (null: no list on screen). Another list than the last starts with an empty draft. */
    fun show(kind: ListKind?) {
        if (kind != null) {
            if (lastShown != null && kind != lastShown) {
                savedState[DRAFT_KEY] = ""
                overrides.value = null
            }
            lastShown = kind
        }
        shown.value = kind
        core.setListShown(kind)
    }

    fun editDraft(text: String) {
        savedState[DRAFT_KEY] = text.replace('\n', ' ')
    }

    fun questOptions(query: String) = (core.read { c, now -> c.taskQuestOptions(query, now) } as? Outcome.Done)?.value.orEmpty()

    fun suggestions(query: String) = (core.read { c, now -> c.quickAddQuestSuggestions(query, now) } as? Outcome.Done)?.value.orEmpty()

    fun calendar(cursor: String?, fields: QuickFields, target: DateTarget) = core.readCalendar(cursor, fields, target)

    fun shiftMonth(cursor: String, forward: Boolean) = core.shiftTaskMonth(cursor, forward)

    fun setQuest(text: String, questId: String?) = changeFields(text) { it.copy(questId = questId) }

    fun setDate(target: DateTarget, date: String?) = changeFields(draft.value) { updateQuickDate(it, target, date) }

    private fun changeFields(text: String, change: (QuickFields) -> QuickFields) {
        val kind = shown.value ?: return
        val result = core.read { c, now ->
            val preview = c.previewQuickAdd(text, overrides.value ?: c.listQuickDefaults(kind, now), now)
            preview.title to change(preview.fields)
        }
        if (result is Outcome.Done) {
            overrides.value = result.value.second
            editDraft(result.value.first)
        }
        report(result)
    }

    /** Adds the draft with the same defaults and parser used by the live preview. */
    fun add() {
        val kind = shown.value ?: return
        val title = draft.value
        if (title.isBlank()) return
        val outcome = core.perform { c, now -> c.addQuickDraft(title, "", overrides.value ?: c.listQuickDefaults(kind, now), now) }
        report(outcome)
        if (outcome is Outcome.Done) {
            overrides.value = null
            savedState[DRAFT_KEY] = ""
        }
    }

    fun toggleTask(taskId: String) = report(core.perform { c, now -> c.toggleListTask(taskId, now) })

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

    private fun listState(state: CoreState, shown: ListKind?, draft: String, defaults: QuickFields?): ListUiState = when (state) {
        CoreState.Opening -> ListUiState.Loading
        is CoreState.Failed -> ListUiState.Failed(state.message)
        is CoreState.Ready -> when (val page = state.list?.takeIf { it.kind == shown }) {
            null -> ListUiState.Loading
            is ListPage.Gone -> ListUiState.Gone
            is ListPage.Shown -> {
                val quick = core.read { c, now ->
                    val fields = c.previewQuickAdd(draft, defaults ?: c.listQuickDefaults(page.kind, now), now).fields
                    val quest = page.list.quest?.takeIf { it.id == fields.questId }
                        ?: fields.questId?.let { id -> c.taskQuestOptions("", now).find { it.id == id } }
                    QuickAddFields(fields, quest, fields.plannedOn?.let(c::quickDayLabel), fields.dueOn?.let(c::quickDayLabel))
                }
                ListUiState.Ready(page.list, draft, (quick as? Outcome.Done)?.value)
            }
        }
    }
}
