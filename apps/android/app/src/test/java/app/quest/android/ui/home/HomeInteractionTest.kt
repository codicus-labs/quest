package app.quest.android.ui.home

import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeRight
import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeHomePreferences
import app.quest.android.FakeRepeatRules
import app.quest.android.Samples
import app.quest.android.core.CoreHolder
import app.quest.android.ui.focus.FocusViewModel
import app.quest.android.ui.homeActions
import app.quest.android.ui.task.TaskSheetState
import app.quest.android.ui.task.TaskSheetViewModel
import app.quest.android.ui.theme.QuestTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Home's gestures and taps, through the real screen and view model over a [FakeCore]. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class HomeInteractionTest {
    @get:Rule val compose = createComposeRule()

    private val core = FakeCore()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

    @After
    fun tearDown() = scope.cancel()

    private lateinit var focus: FocusViewModel
    private lateinit var sheet: TaskSheetViewModel

    private fun showHome(): HomeViewModel {
        val holder = CoreHolder(scope, Dispatchers.Unconfined, FakeClock()) { core }
        val home = HomeViewModel(holder, SavedStateHandle(), FakeHomePreferences())
        sheet = TaskSheetViewModel(holder, SavedStateHandle(), FakeRepeatRules(), FakeClock())
        focus = FocusViewModel(holder, SavedStateHandle())
        compose.setContent {
            QuestTheme(dark = false) {
                val state by home.state.collectAsState()
                HomeScreen(state, homeActions(home, sheet, focus))
            }
        }
        return home
    }

    @Test
    fun swipeRightTogglesExactlyOnce() {
        showHome()

        compose.onNodeWithTag("task:Edit chapter 2").performTouchInput { swipeRight() }
        compose.waitForIdle()

        assertEquals(listOf("t4"), core.toggled)
    }

    @Test
    fun swipeLeftDeletesExactlyOnce() {
        showHome()

        compose.onNodeWithTag("task:Edit chapter 2").performTouchInput { swipeLeft() }
        compose.waitForIdle()

        assertEquals(listOf("delete t4"), core.calls)
    }

    @Test
    fun tappingATitleOpensTheTaskSheetNotAnInlineField() {
        showHome()

        compose.onNodeWithTag("task-title:Outline chapter 3", useUnmergedTree = true).performClick()
        compose.waitForIdle()

        assertEquals("t1", (sheet.state.value as TaskSheetState.Open).detail.id)
        compose.onAllNodesWithTag("task-title-field").assertCountEquals(0)
        assertEquals(emptyList<String>(), core.calls)
    }

    @Test
    fun tappingTheRunningTimeOpensFocusOnThatTask() {
        showHome()

        compose.onNodeWithTag("task-focus:Outline chapter 3").performClick()
        compose.waitForIdle()

        assertEquals(true, focus.state.value != app.quest.android.ui.focus.FocusUiState.Hidden)
        assertEquals("t1", core.focusTasks.last())
    }

    @Test
    fun startingATimerOnACardMakesItFocussTask() {
        showHome()

        compose.onNodeWithTag("timer-start:Edit chapter 2").performClick()
        focus.open()
        compose.waitForIdle()

        assertEquals(listOf("start t4"), core.calls)
        assertEquals("t4", core.focusTasks.last())
    }

    @Test
    fun theAddFieldsIconSwitchesTopAndBottom() {
        val home = showHome()

        compose.onNodeWithTag("add-position-quest_work").performClick()
        compose.waitForIdle()

        assertEquals(uniffi.quest_mobile.AddPosition.BOTTOM, (home.state.value as HomeUiState.Ready).prefs.addPosition)
    }

    @Test
    fun theFilterIconFiltersByTheQuestPickedInThePicker() {
        val home = showHome()

        compose.onNodeWithTag("quest-filter").performClick()
        compose.onNodeWithTag("quest-picker-option:Fitness").performClick()
        compose.waitForIdle()

        assertEquals("q2", (home.state.value as HomeUiState.Ready).inputs.questFilter)
        assertEquals("q2", core.queries.last().questFilter)
    }

    @Test
    fun theFilterChipClearsTheFilter() {
        core.home = core.home.copy(questFilter = Samples.fitness, addQuest = null)
        val home = showHome()
        home.setQuestFilter("q2")
        compose.waitForIdle()

        compose.onNodeWithTag("quest-filter-chip").performClick()
        compose.waitForIdle()

        assertEquals(null, (home.state.value as HomeUiState.Ready).inputs.questFilter)
    }

    @Test
    fun theAddFieldsChipPicksTheQuestForNewQuestWork() {
        val home = showHome()

        compose.onNodeWithTag("add-quest-chip").performClick()
        compose.onNodeWithTag("quest-picker-none").assertDoesNotExist()
        compose.onNodeWithTag("quest-picker-option:Fitness").performClick()
        compose.waitForIdle()

        assertEquals("q2", (home.state.value as HomeUiState.Ready).inputs.questPick)
    }
}
