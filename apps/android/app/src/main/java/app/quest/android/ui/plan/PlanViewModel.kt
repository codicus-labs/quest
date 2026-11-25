package app.quest.android.ui.plan

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.Clock
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.PlanQuery
import app.quest.android.core.PlanRules
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
import uniffi.quest_mobile.MoveTarget
import uniffi.quest_mobile.PlanData
import uniffi.quest_mobile.PlanWindow

/**
 * What the user typed or picked on the plan. Kept in the [SavedStateHandle], so it survives
 * rotation and process death.
 */
data class PlanInputs(
    val questFilter: String? = null,
    /** The first day on screen (ISO), to come back to the same place. */
    val firstVisible: String? = null,
    /** Add-field drafts by day (ISO); empty ones are dropped. */
    val drafts: Map<String, String> = emptyMap(),
) : Serializable {
    fun draft(day: String) = drafts[day].orEmpty()

    fun withDraft(day: String, text: String) = copy(drafts = if (text.isEmpty()) drafts - day else drafts + (day to text))
}

/**
 * "Scroll [day] into view" (null: today), once; [id] tells two requests for the same day apart.
 * [animate]: glide there (Today, a picked date) rather than jump (coming back to the plan).
 */
data class ScrollRequest(val day: String?, val id: Int, val animate: Boolean)

/** The open "Move to…" sheet: its task and the core's targets. */
data class MoveSheet(val taskId: String, val title: String, val targets: List<MoveTarget>)

sealed interface PlanUiState {
    data object Loading : PlanUiState

    data class Ready(
        val plan: PlanData,
        val prefs: HomePrefs,
        val inputs: PlanInputs,
        val moveSheet: MoveSheet?,
        val scroll: ScrollRequest?,
    ) : PlanUiState

    data class Failed(val message: String) : PlanUiState
}

private const val INPUTS_KEY = "plan.inputs"

/**
 * The plan: which window of days the core builds (from what the screen reports as visible),
 * the quest filter, per-day drafts, and every card action. Days, labels and order come from the core.
 */
class PlanViewModel(
    private val core: CoreHolder,
    private val savedState: SavedStateHandle,
    private val preferences: HomePreferences,
    private val rules: PlanRules,
    private val clock: Clock,
) : ViewModel() {
    private val inputs: StateFlow<PlanInputs> = savedState.getStateFlow(INPUTS_KEY, PlanInputs())
    private val prefs: StateFlow<HomePrefs?> = preferences.prefs.stateIn(viewModelScope, SharingStarted.Eagerly, null)
    private val moveSheet = MutableStateFlow<MoveSheet?>(null)
    private val scroll = MutableStateFlow<ScrollRequest?>(null)
    private var scrollIds = 0

    /** The window the core builds; null until the first one is chosen. */
    private var built: PlanWindow? = null
    private var visibleDays = 1
    private var shown = false

    private val _message = MutableStateFlow<String?>(null)

    /** A failed action, until the user dismisses it. */
    val message: StateFlow<String?> = _message.asStateFlow()

    val state: StateFlow<PlanUiState> = combine(core.state, prefs, inputs, moveSheet, scroll) { core, prefs, inputs, sheet, scroll ->
        planState(core, prefs, inputs, sheet, scroll)
    }.stateIn(viewModelScope, SharingStarted.Eagerly, PlanUiState.Loading)

    private fun updateInputs(change: (PlanInputs) -> PlanInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    /** The plan came on screen or left it; the core only builds its days while it shows. */
    fun setShown(shown: Boolean) {
        this.shown = shown
        if (!shown) return core.setPlanQuery(null)
        // Back to the day shown before (another tab, rotation, process death), else today.
        scroll.value = ScrollRequest(inputs.value.firstVisible, scrollIds++, animate = false)
        rebuild(inputs.value.firstVisible, fresh = built == null)
    }

    /** The screen shows [visibleDays] days from [firstVisible] (ISO); builds a new window when needed. */
    fun onVisibleRange(firstVisible: String, visibleDays: Int) {
        this.visibleDays = visibleDays.coerceAtLeast(1)
        if (firstVisible != inputs.value.firstVisible) updateInputs { it.copy(firstVisible = firstVisible) }
        rebuild(firstVisible, fresh = false)
    }

    private fun rebuild(firstVisible: String?, fresh: Boolean) {
        val window = try {
            rules.window(firstVisible, visibleDays, if (fresh) null else built, clock.now())
        } catch (e: Exception) {
            _message.value = e.message ?: "Could not show these days."
            return
        }
        built = window
        if (shown) core.setPlanQuery(PlanQuery(window, inputs.value.questFilter))
    }

    /** Jumps to [day] (ISO; null: today): builds its window and asks the screen to scroll there. */
    fun jumpTo(day: String?) {
        rebuild(day, fresh = true)
        scroll.value = ScrollRequest(day, scrollIds++, animate = true)
    }

    /** The screen scrolled to [request]'s day. */
    fun scrolled(request: ScrollRequest) {
        scroll.compareAndSet(request, null)
    }

    fun setQuestFilter(questId: String?) {
        updateInputs { it.copy(questFilter = questId) }
        built?.let { if (shown) core.setPlanQuery(PlanQuery(it, questId)) }
    }

    fun setShowOther(show: Boolean) {
        viewModelScope.launch { preferences.setShowOther(show) }
    }

    fun setAddPosition(position: AddPosition) {
        viewModelScope.launch { preferences.setAddPosition(position) }
    }

    fun editDraft(day: String, text: String) = updateInputs { it.withDraft(day, text.replace('\n', ' ')) }

    /** Adds [day]'s draft as a task without a quest (desktop plan), at the saved position. */
    fun addTask(day: String) {
        val title = inputs.value.draft(day)
        if (title.isBlank()) return
        val position = prefs.value?.addPosition ?: return
        val outcome = core.perform { c, now -> c.planAddTask(day, title, position, now) }
        report(outcome)
        if (outcome is Outcome.Done) updateInputs { it.withDraft(day, "") }
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

    /** A drop: [taskId] to [day] before [beforeTaskId] (null: the end of its section). */
    fun moveTask(taskId: String, day: String?, beforeTaskId: String?) =
        report(core.perform { c, now -> c.planMoveTask(taskId, day, beforeTaskId, now) })

    /** "Move up" / "Move down" within its day and section. */
    fun nudgeTask(taskId: String, up: Boolean) = report(core.perform { c, now -> c.planNudgeTask(taskId, up, now) })

    /** "Move to previous day" / "Move to next day". */
    fun shiftTask(taskId: String, later: Boolean) = report(core.perform { c, now -> c.planShiftTask(taskId, later, now) })

    /** Opens "Move to…" for [taskId] with the core's targets. */
    fun openMoveTo(taskId: String, title: String) {
        when (val outcome = core.read { c, now -> c.moveTargets(now) }) {
            is Outcome.Done -> moveSheet.value = MoveSheet(taskId, title, outcome.value)
            else -> report(outcome)
        }
    }

    /** Moves the open sheet's task to the end of [day] (null: backlog) and closes the sheet. */
    fun moveTo(day: String?) {
        val sheet = moveSheet.value ?: return
        closeMoveTo()
        moveTask(sheet.taskId, day, null)
    }

    fun closeMoveTo() {
        moveSheet.value = null
    }

    fun dismissMessage() {
        _message.value = null
    }

    private fun report(outcome: Outcome<*>) {
        outcome.failureMessage()?.let { _message.value = it }
    }
}

private fun planState(state: CoreState, prefs: HomePrefs?, inputs: PlanInputs, sheet: MoveSheet?, scroll: ScrollRequest?): PlanUiState = when (state) {
    CoreState.Opening -> PlanUiState.Loading
    is CoreState.Failed -> PlanUiState.Failed(state.message)
    is CoreState.Ready -> {
        val plan = state.plan
        if (plan == null || prefs == null) PlanUiState.Loading else PlanUiState.Ready(plan, prefs, inputs, sheet, scroll)
    }
}
