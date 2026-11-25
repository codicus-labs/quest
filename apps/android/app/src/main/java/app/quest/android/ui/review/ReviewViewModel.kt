package app.quest.android.ui.review

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.QuestCoreApi
import app.quest.android.core.failureMessage
import app.quest.android.core.readCalendar
import app.quest.android.core.shiftTaskMonth
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.DateTarget
import uniffi.quest_mobile.Moment
import uniffi.quest_mobile.PlanTaskList
import uniffi.quest_mobile.QuickFields
import uniffi.quest_mobile.WeeklyReviewData

sealed interface ReviewUiState {
    data object Loading : ReviewUiState
    data class Ready(val review: WeeklyReviewData) : ReviewUiState
    data class Failed(val message: String) : ReviewUiState
}

private const val OFFSET_KEY = "review.offset"

class ReviewViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private var shown = false
    private val offset: Int get() = savedState[OFFSET_KEY] ?: 0
    private val failure = MutableStateFlow<String?>(null)
    val message = failure.asStateFlow()

    val state = core.state.map(::reviewState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, ReviewUiState.Loading)

    fun setShown(shown: Boolean) {
        this.shown = shown
        core.setReviewOffset(if (shown) offset else null)
    }

    fun changeWeek(delta: Int) {
        savedState[OFFSET_KEY] = offset + delta
        if (shown) core.setReviewOffset(offset)
    }

    fun actions(openTask: (String) -> Unit) = ReviewActions(
        previous = { changeWeek(-1) },
        next = { changeWeek(1) },
        openTask = openTask,
        fields = ::fields,
        calendar = core::readCalendar,
        shiftMonth = core::shiftTaskMonth,
        setDate = { id, target, day ->
            act { c, now ->
                when (target) {
                    DateTarget.PLAN -> c.setTaskPlanned(id, day, now)
                    DateTarget.DEADLINE -> c.setTaskDue(id, day, now)
                }
            }
        },
        moveToInbox = { id -> act { c, now -> c.placeTask(id, PlanTaskList.Inbox, null, now) } },
        delete = { id -> act { c, now -> c.deleteTask(id, now) } },
    )

    fun dismissMessage() {
        failure.value = null
    }

    private fun fields(id: String): QuickFields? {
        val outcome = core.read { c, now ->
            val task = c.taskDetail(id, now)
            QuickFields(task.quest?.id, task.plannedOn?.iso, task.dueOn?.iso)
        }
        outcome.failureMessage()?.let { failure.value = it }
        return (outcome as? Outcome.Done)?.value
    }

    private fun act(action: (QuestCoreApi, Moment) -> Unit) {
        core.perform(action).failureMessage()?.let { failure.value = it }
    }
}

private fun reviewState(state: CoreState): ReviewUiState = when (state) {
    CoreState.Opening -> ReviewUiState.Loading
    is CoreState.Failed -> ReviewUiState.Failed(state.message)
    is CoreState.Ready -> state.review?.let { ReviewUiState.Ready(it) } ?: ReviewUiState.Loading
}
