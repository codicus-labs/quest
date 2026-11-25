package app.quest.android.ui.review

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeRight
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeFocusPreferences
import app.quest.android.ListSamples
import app.quest.android.QuestSamples
import app.quest.android.core.CoreHolder
import app.quest.android.core.QuestCoreApi
import app.quest.android.ui.QuestApp
import app.quest.android.ui.testAppViewModels
import app.quest.android.ui.theme.QuestTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.emptyFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.quest_mobile.CalendarMonth
import uniffi.quest_mobile.DateChoice
import uniffi.quest_mobile.DateTarget
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.Moment
import uniffi.quest_mobile.PlayerCard
import uniffi.quest_mobile.QuickFields
import uniffi.quest_mobile.UndoToast

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class ReviewFlowTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private val clock = FakeClock()
    private val core = FakeCore()
    private val initial = QuestSamples.review.mostPostponed.first()
    private val tomorrow = DayValue("2026-11-24", "Nov 24, 2026")
    private var rankedTask = initial

    private val reviewCore = object : QuestCoreApi by core {
        override fun weeklyReview(offset: Int, now: Moment) = core.weeklyReview(offset, now).let { review ->
            if (offset == 0) review.copy(mostPostponed = listOfNotNull(rankedTask.takeIf { it.taskId in core.details }) + review.mostPostponed.drop(1))
            else review
        }

        override fun taskCalendar(cursor: String?, fields: QuickFields, target: DateTarget, now: Moment): CalendarMonth {
            assertEquals(DateTarget.PLAN, target)
            return CalendarMonth(tomorrow, "November 2026", listOf(DateChoice("Tomorrow", tomorrow)), emptyList())
        }

        override fun setTaskPlanned(taskId: String, plannedOn: String?, now: Moment) {
            core.setTaskPlanned(taskId, plannedOn, now)
            rankedTask = rankedTask.copy(count = rankedTask.count + 1u)
        }

        private var deletedDetail = core.details[initial.taskId]

        override fun deleteTask(taskId: String, now: Moment) {
            deletedDetail = core.details[taskId]
            core.deleteTask(taskId, now)
            core.home = core.home.copy(toast = UndoToast("Deleted “${initial.title}”", "Undo", now.epochMs + 6_000))
        }

        override fun undo(now: Moment) {
            core.undo(now)
            deletedDetail?.let { core.details[initial.taskId] = it }
        }

        override fun stopTimer(now: Moment) {
            core.stopTimer(now)
            core.player = PlayerCard.Empty
        }
    }

    @After
    fun tearDown() = scope.cancel()

    private fun showApp() {
        val holder = CoreHolder(scope, Dispatchers.Unconfined, clock) { reviewCore }
        val viewModels = testAppViewModels(holder)
        compose.setContent { QuestTheme(dark = false) { QuestApp(viewModels, emptyFlow(), FakeFocusPreferences()) } }
    }

    private fun openReview() {
        showApp()
        compose.onNodeWithTag("tab-tracked").performClick()
        compose.onNodeWithTag("tracked-review").performClick()
    }

    @Test
    fun arrowsAndSwipeChangeTheWeek() {
        openReview()
        compose.onNodeWithText("Nov 23 – 29").assertIsDisplayed()
        compose.onNodeWithTag("review-previous").performClick()
        compose.onNodeWithText("Nov 16 – 22").assertIsDisplayed()
        compose.onNodeWithTag("review-next").performClick()
        compose.onNodeWithText("Nov 23 – 29").assertIsDisplayed()
        compose.onNodeWithTag("review-pages").performTouchInput { swipeRight() }
        compose.onNodeWithText("Nov 16 – 22").assertIsDisplayed()
        assertEquals(listOf(0, -1, 0, -1), core.reviewOffsets.fold(emptyList<Int>()) { weeks, offset ->
            if (weeks.lastOrNull() == offset) weeks else weeks + offset
        })
    }

    @Test
    @Config(fontScale = 2f)
    fun historyPrecedesActionsAndTomorrowReloadsTheCountAtLargeFont() {
        openReview()
        val schedule = compose.onNodeWithTag("review-plan:t1").performScrollTo()
        val plan = compose.onNodeWithText("Plan · Nov 20", substring = true)
        val deadline = compose.onNodeWithText("Deadline · Nov 27", substring = true)
        assertTrue(plan.getUnclippedBoundsInRoot().bottom <= deadline.getUnclippedBoundsInRoot().top)
        assertTrue(deadline.getUnclippedBoundsInRoot().bottom <= schedule.getUnclippedBoundsInRoot().top)

        schedule.performClick()
        compose.onNodeWithText("Tomorrow").performClick()
        compose.onNodeWithTag("review-count:t1").assertTextEquals("↷ 5")
        assertEquals(listOf("day t1 2026-11-24"), core.calls)
        assertEquals(0, core.reviewOffsets.last())
    }

    @Test
    fun tappingATitleOpensTheTaskSheet() {
        openReview()
        compose.onNodeWithText(initial.title).performClick()
        compose.onNodeWithTag("task-sheet").assertIsDisplayed()
    }

    @Test
    fun deleteAndUndoRefreshTheSameWeek() {
        openReview()
        compose.onNodeWithTag("review-delete:t1").performClick()
        compose.onNodeWithText(initial.title).assertDoesNotExist()
        compose.onNodeWithTag("snackbar-action").assertIsDisplayed().performClick()
        compose.onNodeWithText(initial.title).assertIsDisplayed()
        compose.onNodeWithText("Deleted “${initial.title}”").assertDoesNotExist()
        assertEquals(listOf("delete t1", "undo"), core.calls)
        assertEquals(listOf(0), core.reviewOffsets.distinct())
    }

    @Test
    fun organizingStopLetsTheCoreDiscardAShortSessionSilently() {
        core.player = ListSamples.playerOrganizing.copy(timer = "00:12")
        showApp()
        compose.onNodeWithTag("tab-tracked").performClick()
        compose.onNodeWithTag("player-stop").performClick()
        compose.onNodeWithTag("player").assertDoesNotExist()
        compose.onNodeWithTag("snackbar").assertDoesNotExist()
        assertEquals(listOf("stop"), core.calls)
    }
}
