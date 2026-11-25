package app.quest.android.ui.search

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
import uniffi.quest_mobile.SearchMode
import uniffi.quest_mobile.SearchOutcome
import uniffi.quest_mobile.SearchResult
import uniffi.quest_mobile.SearchToken

/** [SearchMode] in a form SavedStateHandle can keep. */
sealed interface SavedSearchMode : Serializable {
    data object Commands : SavedSearchMode

    data class MoveTo(val taskId: String) : SavedSearchMode

    data class ScheduleAt(val taskId: String) : SavedSearchMode

    data object SwitchWorkspace : SavedSearchMode
}

fun SavedSearchMode.toCore(): SearchMode = when (this) {
    SavedSearchMode.Commands -> SearchMode.Commands
    is SavedSearchMode.MoveTo -> SearchMode.MoveTo(taskId)
    is SavedSearchMode.ScheduleAt -> SearchMode.ScheduleAt(taskId)
    SavedSearchMode.SwitchWorkspace -> SearchMode.SwitchWorkspace
}

fun SearchMode.toSaved(): SavedSearchMode = when (this) {
    SearchMode.Commands -> SavedSearchMode.Commands
    is SearchMode.MoveTo -> SavedSearchMode.MoveTo(taskId)
    is SearchMode.ScheduleAt -> SavedSearchMode.ScheduleAt(taskId)
    SearchMode.SwitchWorkspace -> SavedSearchMode.SwitchWorkspace
}

/** What the user typed and which list they are in; survives process death. */
data class SearchInputs(val query: String = "", val mode: SavedSearchMode = SavedSearchMode.Commands) : Serializable

sealed interface SearchUiState {
    data object Loading : SearchUiState

    data class Ready(val query: String, val mode: SavedSearchMode, val placeholder: String, val results: List<SearchResult>) : SearchUiState

    data class Failed(val message: String) : SearchUiState
}

private const val INPUTS_KEY = "search.inputs"

/**
 * Search: the core lists results for the query and mode; running one returns a [SearchOutcome]
 * the app carries out. A choice (move to, schedule at) switches the mode and stays open.
 */
class SearchViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private val inputs: StateFlow<SearchInputs> = savedState.getStateFlow(INPUTS_KEY, SearchInputs())

    /** Searches run only while search is on screen (not on every tick behind it). */
    private val shown = MutableStateFlow(false)

    val state: StateFlow<SearchUiState> = combine(core.state, inputs, shown, ::searchState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, SearchUiState.Loading)

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    private fun searchState(state: CoreState, inputs: SearchInputs, shown: Boolean): SearchUiState = when {
        !shown -> SearchUiState.Loading
        else -> searchState(state, inputs)
    }

    private fun searchState(state: CoreState, inputs: SearchInputs): SearchUiState = when (state) {
        CoreState.Opening -> SearchUiState.Loading
        is CoreState.Failed -> SearchUiState.Failed(state.message)
        is CoreState.Ready -> {
            val mode = inputs.mode.toCore()
            when (val read = core.read { c, now -> c.searchPlaceholder(mode) to c.search(mode, inputs.query, now) }) {
                is Outcome.Done -> SearchUiState.Ready(inputs.query, inputs.mode, read.value.first, read.value.second)
                else -> SearchUiState.Failed(read.failureMessage() ?: "")
            }
        }
    }

    fun setShown(shown: Boolean) {
        this.shown.value = shown
    }

    fun editQuery(query: String) {
        savedState[INPUTS_KEY] = inputs.value.copy(query = query.replace('\n', ' '))
    }

    /** Back to the command list with an empty field (each time search opens or closes). */
    fun reset() {
        savedState[INPUTS_KEY] = SearchInputs()
    }

    /** Runs [token]; returns what the app should do next, or null when it stays open (a choice, a failure). */
    fun run(token: SearchToken): SearchOutcome? {
        val outcome = core.perform { c, now -> c.runSearch(token, now) }
        if (outcome !is Outcome.Done) {
            outcome.failureMessage()?.let { _message.value = it }
            return null
        }
        val result = outcome.value
        if (result is SearchOutcome.Choose) {
            savedState[INPUTS_KEY] = SearchInputs(query = "", mode = result.mode.toSaved())
            return null
        }
        reset()
        return result
    }

    /** The keyboard's action: runs the first result. */
    fun runFirst(): SearchOutcome? {
        val first = (state.value as? SearchUiState.Ready)?.results?.firstOrNull() ?: return null
        return run(first.token)
    }

    fun dismissMessage() {
        _message.value = null
    }
}
