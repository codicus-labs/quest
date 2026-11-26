package app.quest.android.ui

import androidx.activity.ComponentActivity
import androidx.lifecycle.Lifecycle
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.click
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import app.quest.android.FakeClock
import app.quest.android.ListSamples
import uniffi.quest_mobile.FocusExitStatus
import app.quest.android.FakeFocusPreferences
import app.quest.android.FakeCore
import app.quest.android.core.CoreHolder
import app.quest.android.ui.task.TaskSheetState
import app.quest.android.ui.theme.QuestTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.emptyFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Focus over the whole app: Home under it must not react. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class QuestAppFocusTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    private val core = FakeCore()
    private val clock = FakeClock()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

    @After
    fun tearDown() = scope.cancel()

    private fun showApp(): CoreHolder {
        val holder = CoreHolder(scope, Dispatchers.Unconfined, clock) { core }
        val viewModels = testAppViewModels(holder)
        compose.setContent { QuestTheme(dark = false) { QuestApp(viewModels, emptyFlow(), FakeFocusPreferences()) } }
        models = viewModels
        return holder
    }

    private lateinit var models: AppViewModels

    @Test
    fun backgroundKeepsFocusOpenButLeavingReportsTheTime() {
        showApp()
        compose.onNodeWithTag("player").performClick()
        compose.onNodeWithTag("focus").assertExists()

        compose.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
        compose.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        compose.onNodeWithTag("focus").assertExists()
        assertEquals(1, core.focusOpenedCount)
        assertEquals(emptyList<uniffi.quest_mobile.Moment>(), core.focusDepartures)

        clock.epochMs += 5_000
        compose.onNodeWithTag("focus-back").performClick()
        compose.onNodeWithTag("focus").assertDoesNotExist()
        assertEquals(listOf(clock.now()), core.focusDepartures)
    }

    @Test
    fun startingFromAListOpensFocusAndOrganizeReturnsToTheList() {
        showApp()
        compose.onNodeWithTag("timer-start:Sketch a pricing page").performClick()
        compose.onNodeWithTag("focus").assertExists()
        compose.onNodeWithTag("focus-organize").performClick()

        compose.onNodeWithTag("list-today").assertExists()
        assertEquals(listOf("start t1", "organize"), core.calls)
        assertEquals(emptyList<uniffi.quest_mobile.Moment>(), core.focusDepartures)
    }

    @Test
    fun backToTaskUsesTheCoreAndOpensFocus() {
        core.player = ListSamples.playerOrganizing
        showApp()
        compose.onNodeWithTag("player-back-to").performClick()

        compose.onNodeWithTag("focus").assertExists()
        assertEquals(1, core.focusOpenedCount)
        assertEquals(listOf("back to task"), core.calls)
        assertEquals(ListSamples.playerOrganizing.backToTaskId, core.focusTasks.last())
    }

    @Test
    fun bookShowsUndoOnTracked() {
        val holder = showApp()
        compose.onNodeWithTag("tab-tracked").performClick()
        compose.onNodeWithTag("organizing-hint").assertDoesNotExist()
        core.exitStatus = FocusExitStatus("e4", clock.now().epochMs - 120_000, true, "14:18")
        holder.setForeground(true)
        compose.onNodeWithText("Book since 14:18").performClick()

        compose.onNodeWithTag("organizing-hint").assertDoesNotExist()
        compose.onNodeWithText("Booked as Organizing").assertExists()
        compose.onNodeWithText("Undo").performClick()

        compose.onNodeWithText("Booked as Organizing").assertDoesNotExist()
        assertEquals(listOf("book focus time", "undo"), core.calls)
    }

    @Test
    @Config(fontScale = 2f)
    fun longCompactTimesStayVisibleWithLargeText() {
        core.player = ListSamples.playerOrganizing.copy(timer = "99h 59m")
        core.tracked = ListSamples.tracked.copy(total = "18h 38m")
        showApp()

        compose.onNodeWithText("99h 59m", useUnmergedTree = true).assertIsDisplayed()
        compose.onNodeWithTag("player-stop", useUnmergedTree = true).assertIsDisplayed()
        compose.onNodeWithTag("tab-tracked").performClick()
        compose.onNodeWithText("18h 38m today").assertIsDisplayed()
    }

    @Test
    fun addsAndRenamesAChecklistItemInFocus() {
        showApp()
        compose.onNodeWithTag("player").performClick()
        compose.onNodeWithTag("list-add-open").assertDoesNotExist()
        val add = compose.onNodeWithTag("checklist-add").performScrollTo()
        add.performTextInput("Draft an outline")
        add.performImeAction()
        val item = compose.onNodeWithTag("checklist-text-new-item").performScrollTo()
        item.performTextReplacement("Outline chapter one")
        add.performClick()

        compose.runOnIdle {
            assertEquals(
                listOf("add checklist ${app.quest.android.Samples.focusSession.taskId} Draft an outline", "rename checklist new-item Outline chapter one"),
                core.calls,
            )
        }
    }

    @Test
    fun tapsOnFocusNeverReachHome() {
        showApp()
        val sheet = models.taskSheet
        val focus = models.focus
        compose.onNodeWithTag("list-today").assertExists()

        focus.open()
        compose.waitForIdle()
        compose.onNodeWithTag("list-today").assertDoesNotExist()
        // Where Home's cards are: an empty part of Focus.
        compose.onRoot().performTouchInput {
            click(Offset(centerX, height * 0.5f))
            click(Offset(centerX, height * 0.85f))
        }
        compose.waitForIdle()

        assertEquals(emptyList<String>(), core.calls)
        assertEquals(emptyList<String>(), core.toggled)
        assertEquals(TaskSheetState.Closed, sheet.state.value)
    }
}
