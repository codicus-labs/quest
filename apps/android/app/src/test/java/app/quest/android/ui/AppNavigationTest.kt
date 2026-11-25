package app.quest.android.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextInput
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeFocusPreferences
import app.quest.android.core.CoreHolder
import app.quest.android.ui.navigation.TopDestination
import app.quest.android.settings.Appearance
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
import uniffi.quest_mobile.AppView
import uniffi.quest_mobile.SearchOutcome
import uniffi.quest_mobile.SearchToken

/** Search, the archive and the quest page, driven through the whole app. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class AppNavigationTest {
    @get:Rule val compose = createComposeRule()

    private val core = FakeCore()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private val appearance = FixedAppearance()

    @After
    fun tearDown() = scope.cancel()

    private fun showApp(): AppViewModels {
        val holder = CoreHolder(scope, Dispatchers.Unconfined, FakeClock()) { core }
        val viewModels = testAppViewModels(holder, appearance)
        compose.setContent { QuestTheme(dark = false) { QuestApp(viewModels, emptyFlow(), FakeFocusPreferences()) } }
        return viewModels
    }

    private fun search(query: String) {
        compose.onNodeWithTag("open-search").performClick()
        compose.onNodeWithTag("search-query").performTextInput(query)
        compose.waitForIdle()
    }

    @Test
    fun everyTabHasAHeadingForScreenReaders() {
        showApp()

        val tabsWithoutHeading = TopDestination.entries.filter { tab ->
            compose.onNodeWithTag("tab-${tab.name.lowercase()}").performClick()
            compose.waitForIdle()
            compose.onAllNodes(isHeading()).fetchSemanticsNodes().isEmpty()
        }

        assertEquals(emptyList<TopDestination>(), tabsWithoutHeading)
    }

    @Test
    fun aJumpFromSearchOpensItsTab() {
        core.searchOutcome = { token -> SearchOutcome.Show((token as SearchToken.Go).view) }
        showApp()

        search("quest")
        compose.onNodeWithTag("search-result:Go to Quests").performClick()

        compose.onNodeWithTag("search").assertDoesNotExist()
        compose.onNodeWithTag("quests-list").assertIsDisplayed()
        assertEquals("run ${SearchToken.Go(AppView.QUESTS)}", core.calls.last())
    }

    @Test
    fun theKeyboardsSearchOpensTheFirstResultsQuestPage() {
        core.searchOutcome = { token -> SearchOutcome.ShowQuest((token as SearchToken.ShowQuest).questId) }
        showApp()

        search("book")
        compose.onNodeWithTag("search-query").performImeAction()

        compose.onNodeWithTag("quest-detail").assertIsDisplayed()
        compose.onNodeWithTag("search").assertDoesNotExist()
    }

    @Test
    fun searchOpensEmptyAgainAfterAResultOpenedAQuestPage() {
        core.searchOutcome = { token -> SearchOutcome.ShowQuest((token as SearchToken.ShowQuest).questId) }
        showApp()
        search("book")
        compose.onNodeWithTag("search-query").performImeAction()
        compose.onNodeWithTag("quest-back").performClick()

        compose.onNodeWithTag("open-search").performClick()

        compose.onNodeWithTag("search-query").assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")))
    }

    @Test
    fun toggleThemeFromSearchSwitchesToTheOtherAppearance() {
        core.searchOutcome = { SearchOutcome.ToggleTheme }
        showApp()

        search("theme")
        compose.onNodeWithTag("search-result:Toggle theme").performClick()
        compose.waitForIdle()

        assertEquals(Appearance.Dark, appearance.appearance.value)
    }

    @Test
    fun restoringFromTheArchiveOpenedOnTheQuestsTab() {
        showApp()
        compose.onNodeWithTag("tab-quests").performClick()
        compose.onNodeWithTag("quests-archive").performClick()

        compose.onNodeWithTag("archive-restore:Old hobby").performClick()

        assertEquals("restore QUEST q9", core.calls.last())
    }

    @Test
    fun logWorkFromSearchOpensTheFormOnTheCalendar() {
        core.searchOutcome = { SearchOutcome.LogWork }
        showApp()

        search("log")
        compose.onNodeWithTag("search-result:Log work").performClick()

        compose.onNodeWithTag("search").assertDoesNotExist()
        compose.onNodeWithTag("calendar-sheet").assertExists()
    }

    @Test
    fun focusFromSearchOpensFocus() {
        core.searchOutcome = { token -> SearchOutcome.Focus((token as SearchToken.Focus).taskId) }
        showApp()

        search("outline")
        compose.onNodeWithTag("search-result:Outline chapter 3").performClick()

        compose.onNodeWithTag("focus").assertExists()
    }

    @Test
    fun accountFromSearchOpensAccount() {
        core.searchOutcome = { SearchOutcome.Account }
        showApp()

        search("quest")
        compose.onNodeWithTag("search-result:Go to Quests").performClick()

        compose.onNodeWithTag("account").assertExists()
    }

    @Test
    fun showArchiveFromSearchOpensTheArchive() {
        core.searchOutcome = { SearchOutcome.Show(AppView.ARCHIVE) }
        showApp()

        search("quest")
        compose.onNodeWithTag("search-result:Go to Quests").performClick()

        compose.onNodeWithTag("archive").assertExists()
    }

    @Test
    fun aDoneOutcomeOnlyClosesSearch() {
        core.searchOutcome = { SearchOutcome.Done }
        showApp()

        search("quest")
        compose.onNodeWithTag("search-result:Go to Quests").performClick()

        compose.onNodeWithTag("search").assertDoesNotExist()
        compose.onNodeWithTag("open-search").assertExists()
    }
}
