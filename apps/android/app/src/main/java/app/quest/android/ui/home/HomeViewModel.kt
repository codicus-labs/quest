package app.quest.android.ui.home

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
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
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.DayGrid
import uniffi.quest_mobile.HomeData
import uniffi.quest_mobile.HomeQuery
import uniffi.quest_mobile.SyncState
import uniffi.quest_mobile.TaskSection

/** What a compact (phone) Home shows; wider windows show both side by side. */
enum class CompactView { List, Grid }

/**
 * What the user typed or picked on Home. Kept in the [SavedStateHandle], so it survives
 * rotation and process death.
 */
data class HomeInputs(
    val questFilter: String? = null,
    val questPick: String? = null,
    val questWorkDraft: String = "",
    val otherDraft: String = "",
    val compactView: CompactView = CompactView.List,
) : Serializable {
    val query get() = HomeQuery(questFilter = questFilter, questPick = questPick)

    fun draft(section: TaskSection) = when (section) {
        TaskSection.QUEST_WORK -> questWorkDraft
        TaskSection.OTHER -> otherDraft
    }

    fun withDraft(section: TaskSection, text: String) = when (section) {
        TaskSection.QUEST_WORK -> copy(questWorkDraft = text)
        TaskSection.OTHER -> copy(otherDraft = text)
    }
}

sealed interface HomeUiState {
    data object Loading : HomeUiState

    data class Ready(
        val home: HomeData,
        val grid: DayGrid,
        val sync: SyncState,
        val prefs: HomePrefs,
        val inputs: HomeInputs,
    ) : HomeUiState

    data class Failed(val message: String) : HomeUiState
}

private const val INPUTS_KEY = "home.inputs"

class HomeViewModel(
    private val core: CoreHolder,
    private val savedState: SavedStateHandle,
    private val preferences: HomePreferences,
) : ViewModel() {
    private val inputs: StateFlow<HomeInputs> = savedState.getStateFlow(INPUTS_KEY, HomeInputs())
    /** Null until DataStore is read, so a hidden OTHER does not flash and an early add uses the saved position. */
    private val prefs: StateFlow<HomePrefs?> = preferences.prefs.stateIn(viewModelScope, SharingStarted.Eagerly, null)

    val state: StateFlow<HomeUiState> = combine(core.state, prefs, inputs, ::homeState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, homeState(core.state.value, prefs.value, inputs.value))

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    init {
        core.setHomeQuery(inputs.value.query)
    }

    private fun updateInputs(change: (HomeInputs) -> HomeInputs) {
        val next = change(inputs.value)
        savedState[INPUTS_KEY] = next
        core.setHomeQuery(next.query)
    }

    fun toggleTask(taskId: String) = report(core.perform { c, now -> c.toggleTask(taskId, now) })

    fun deleteTask(taskId: String) = report(core.perform { c, now -> c.deleteTask(taskId, now) })

    /** Starts [taskId]'s timer (stopping another); true when it started. */
    fun startTimer(taskId: String): Boolean {
        val outcome = core.perform { c, now -> c.startTimer(taskId, now) }
        report(outcome)
        return outcome is Outcome.Done
    }

    fun assignQuest(taskId: String, questId: String) = report(core.perform { c, _ -> c.setTaskQuest(taskId, questId) })

    fun stopTimer() = report(core.perform { c, now -> c.stopTimer(now) })

    fun undo() = report(core.perform { c, now -> c.undo(now) })

    fun dismissToast() = report(core.perform { c, now -> c.dismissToast(now) })

    fun editDraft(section: TaskSection, text: String) = updateInputs { it.withDraft(section, text) }

    /** Adds the section's draft as a task for today; the core picks its quest (desktop rule). */
    fun addTask(section: TaskSection) {
        val current = inputs.value
        val title = current.draft(section)
        if (title.isBlank()) return
        val position = prefs.value?.addPosition ?: return
        val outcome = core.perform { c, now -> c.addTask(title, section, position, current.query, now) }
        report(outcome)
        if (outcome is Outcome.Done) updateInputs { it.withDraft(section, "") }
    }

    /** Filters QUEST WORK by [questId]; null clears the filter. */
    fun setQuestFilter(questId: String?) = updateInputs { it.copy(questFilter = questId) }

    /** The quest new quest work gets while no filter is set. */
    fun pickQuest(questId: String) = updateInputs { it.copy(questPick = questId) }

    /** Another workspace is shown: its quests differ, so the filter and the pick go (typed text stays). */
    fun leaveWorkspace() = updateInputs { it.copy(questFilter = null, questPick = null) }

    fun setCompactView(view: CompactView) = updateInputs { it.copy(compactView = view) }

    fun setShowOther(show: Boolean) {
        viewModelScope.launch { preferences.setShowOther(show) }
    }

    fun setAddPosition(position: AddPosition) {
        viewModelScope.launch { preferences.setAddPosition(position) }
    }

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }
}

private fun homeState(state: CoreState, prefs: HomePrefs?, inputs: HomeInputs): HomeUiState = when (state) {
    CoreState.Opening -> HomeUiState.Loading
    is CoreState.Ready -> if (prefs == null) HomeUiState.Loading else HomeUiState.Ready(state.home, state.todayGrid, state.sync, prefs, inputs)
    is CoreState.Failed -> HomeUiState.Failed(state.message)
}
