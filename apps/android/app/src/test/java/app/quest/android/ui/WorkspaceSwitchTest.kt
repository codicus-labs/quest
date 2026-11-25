package app.quest.android.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import app.quest.android.DEFAULT_WORKSPACE
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeFocusPreferences
import app.quest.android.FakeWorkspaces
import app.quest.android.WorkspaceSamples
import app.quest.android.core.CoreHolder
import app.quest.android.ui.home.HomeUiState
import app.quest.android.ui.theme.QuestTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.emptyFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.quest_mobile.SearchKind
import uniffi.quest_mobile.SearchOutcome
import uniffi.quest_mobile.SearchResult
import uniffi.quest_mobile.SearchToken

/** The switcher, "Manage workspaces" and what a switch resets, driven through the whole app. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class WorkspaceSwitchTest {
    @get:Rule val compose = createComposeRule()

    private val workspaces = FakeWorkspaces(WorkspaceSamples.two)
    private val core = FakeCore(workspaces = workspaces)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

    @After
    fun tearDown() = scope.cancel()

    private fun showApp(): AppViewModels {
        val holder = CoreHolder(scope, Dispatchers.Unconfined, FakeClock()) { onChange -> core.also { it.onChange = onChange } }
        val viewModels = testAppViewModels(holder)
        compose.setContent { QuestTheme(dark = false) { QuestApp(viewModels, emptyFlow(), FakeFocusPreferences()) } }
        compose.waitForIdle()
        return viewModels
    }

    private fun tap(tag: String) {
        compose.onNodeWithTag(tag).performClick()
        compose.waitForIdle()
    }

    @Test
    fun theMenuSwitchesAndTheSwitcherShowsTheNewWorkspace() {
        val viewModels = showApp()
        viewModels.home.setQuestFilter("q1")

        tap("workspace-switcher")
        tap("workspace-menu-item1")

        compose.onNodeWithTag("workspace-name", useUnmergedTree = true).assertTextEquals("Work")
        assertEquals(listOf("switch w2"), workspaces.calls)
        assertNull((viewModels.home.state.value as HomeUiState.Ready).inputs.questFilter)
    }

    @Test
    fun aQuestPageGivesWayToQuestsWhenAnotherDeviceDeletesTheShownWorkspace() {
        val viewModels = showApp()
        viewModels.questDetail.open("q1")
        compose.waitForIdle()
        compose.onNodeWithTag("quest-detail").assertIsDisplayed()

        workspaces.switchWorkspace("w2")
        core.onChange()
        compose.waitForIdle()

        compose.onNodeWithTag("quest-detail").assertDoesNotExist()
        compose.onNodeWithTag("tab-quests").assertIsSelected()
        compose.onNodeWithTag("workspace-name", useUnmergedTree = true).assertTextEquals("Work")
    }

    @Test
    fun manageFromTheMenuAsksBeforeDeletingAndCreateShowsTheNewOne() {
        showApp()
        tap("workspace-switcher")
        tap("workspace-manage")

        tap("workspace-delete-w2")
        compose.onNodeWithTag("workspace-delete-question").assertTextEquals(workspaces.question)
        tap("workspace-delete-confirm")
        compose.onNodeWithTag("workspace-confirm").assertDoesNotExist()

        compose.onNodeWithTag("workspace-new").performTextInput("Side")
        tap("workspace-create")

        assertEquals(listOf("ask delete w2", "delete w2", "create Side"), workspaces.calls)
        compose.onNodeWithTag("workspaces-sheet").assertDoesNotExist()
        compose.onNodeWithTag("workspace-name", useUnmergedTree = true).assertTextEquals("Side")
    }

    @Test
    fun searchOpensManageWorkspaces() {
        val manage = SearchResult("Manage workspaces…", "Action", SearchKind.ACTION, SearchToken.ManageWorkspaces)
        core.searchResults = { _, _ -> listOf(manage) }
        core.searchOutcome = { SearchOutcome.ManageWorkspaces }
        showApp()

        tap("open-search")
        compose.onNodeWithTag("search-query").performTextInput("work")
        compose.waitForIdle()
        tap("search-result:Manage workspaces…")

        compose.onNodeWithTag("search").assertDoesNotExist()
        compose.onNodeWithTag("workspaces-sheet").assertIsDisplayed()
    }

    @Test
    fun theTimerNoteShowsItsWorkspaceOnFocus() {
        workspaces.data = WorkspaceSamples.timerInDefault
        showApp()

        tap("timer-elsewhere")

        assertEquals(listOf("switch $DEFAULT_WORKSPACE"), workspaces.calls)
        compose.onNodeWithTag("focus").assertIsDisplayed()
        compose.onNodeWithTag("timer-elsewhere").assertDoesNotExist()
    }
}
