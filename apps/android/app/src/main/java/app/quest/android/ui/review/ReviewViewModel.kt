package app.quest.android.ui.review

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.ReviewQuery
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.quest_mobile.ReviewData

sealed interface ReviewUiState {
    data object Loading : ReviewUiState

    data class Ready(val review: ReviewData) : ReviewUiState

    data class Failed(val message: String) : ReviewUiState
}

private const val WEEK_KEY = "review.week"

/** The weekly review: which week (any ISO day in it; null this week) is kept; the numbers are the core's. */
class ReviewViewModel(private val core: CoreHolder, private val savedState: SavedStateHandle) : ViewModel() {
    private var shown = false

    val state: StateFlow<ReviewUiState> = core.state.map(::reviewState).stateIn(viewModelScope, SharingStarted.Eagerly, ReviewUiState.Loading)

    private val week: String? get() = savedState[WEEK_KEY]

    fun setShown(shown: Boolean) {
        this.shown = shown
        requery()
    }

    private fun requery() = core.setReviewQuery(if (shown) ReviewQuery(week) else null)

    private fun goTo(week: String?) {
        savedState[WEEK_KEY] = week
        requery()
    }

    private fun shownReview(): ReviewData? = ((state.value as? ReviewUiState.Ready)?.review)

    /** The shown review, once it is the requested week (a second quick tap would otherwise repeat the first). */
    private fun settledReview(): ReviewData? = shownReview()?.takeIf { if (week == null) it.isThisWeek else it.weekStart == week }

    fun previous() = settledReview()?.let { goTo(it.previousWeek) }

    fun next() = settledReview()?.let { goTo(it.nextWeek) }

    fun thisWeek() = goTo(null)

    /** A bar of the recent weeks: shows that week. */
    fun selectWeek(weekStart: String) = goTo(weekStart)
}

private fun reviewState(state: CoreState): ReviewUiState = when (state) {
    CoreState.Opening -> ReviewUiState.Loading
    is CoreState.Failed -> ReviewUiState.Failed(state.message)
    is CoreState.Ready -> state.review?.let { ReviewUiState.Ready(it) } ?: ReviewUiState.Loading
}
