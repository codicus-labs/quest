package app.quest.android.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeFocusPreferences
import app.quest.android.ListSamples
import app.quest.android.QuestSamples
import app.quest.android.core.CoreHolder
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
import uniffi.quest_mobile.ListKind

/** The Quests tab, a quest's list with its editor sheet, and the archive, driven through the whole app. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class QuestsFlowTest {
    @get:Rule val compose = createComposeRule()

    private val core = FakeCore()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

    @After
    fun tearDown() = scope.cancel()

    private fun showQuests(): AppViewModels {
        val holder = CoreHolder(scope, Dispatchers.Unconfined, FakeClock()) { core }
        val viewModels = testAppViewModels(holder)
        compose.setContent { QuestTheme(dark = false) { QuestApp(viewModels, emptyFlow(), FakeFocusPreferences()) } }
        compose.onNodeWithTag("tab-quests").performClick()
        compose.waitForIdle()
        return viewModels
    }

    private fun tap(tag: String) {
        compose.onNodeWithTag(tag).performClick()
        compose.waitForIdle()
    }

    private fun showList(questId: String, title: String) {
        core.lists[ListKind.Quest(questId)] = ListSamples.questList.copy(kind = ListKind.Quest(questId), title = title)
    }

    @Test
    fun aQuestOpensItsListAndBackGoesToTheQuests() {
        showList("q1", "Write the book")
        showQuests()

        tap("quest-row:Write the book")
        compose.onNodeWithTag("list-quest").assertIsDisplayed()
        tap("quests-back")

        compose.onNodeWithTag("quests-list").assertIsDisplayed()
    }

    @Test
    fun tappingTheQuestsTabAgainGoesBackToTheQuests() {
        showQuests()
        tap("quests-logbook")
        compose.onNodeWithTag("list-logbook").assertIsDisplayed()

        tap("tab-quests")

        compose.onNodeWithTag("quests-list").assertIsDisplayed()
    }

    @Test
    fun theEditOnAQuestsListOpensTheEditorAndCompletesTheQuest() {
        showList("q1", "Write the book")
        showQuests()
        tap("quest-row:Write the book")

        tap("quest-edit")
        tap("quest-editor-status")

        assertEquals(listOf("toggle-quest q1"), core.calls)
        compose.onNodeWithText("Completed quest “Write the book”").assertIsDisplayed()
    }

    @Test
    fun newQuestCreatesTheQuestAndItsPageOpensItsList() {
        core.questDetails["q-new"] = QuestSamples.detail.copy(id = "q-new", title = "Learn piano")
        showList("q-new", "Learn piano")
        showQuests()

        tap("quests-new")
        compose.onNode(hasText("New quest") and isHeading()).assertExists()
        compose.onNodeWithTag("quest-editor-title").performTextInput("Learn piano")
        tap("quest-editor-create")
        compose.onNode(hasText("Edit quest") and isHeading()).assertExists()
        tap("quest-editor-page")

        assertEquals(listOf("create-quest Learn piano|null"), core.calls)
        compose.onNodeWithTag("quest-editor-title").assertDoesNotExist()
        compose.onNodeWithTag("list-quest").assertIsDisplayed()
    }

    @Test
    fun anArchivedQuestIsUnarchivedFromTheDrawersArchive() {
        showQuests()
        tap("tab-account")
        tap("open-archive")
        compose.onNodeWithText("Archived quests", ignoreCase = true).assertIsDisplayed()

        tap("archive-unarchive:Old blog")

        assertEquals(listOf("unarchive-quest q8"), core.calls)
        compose.onNodeWithTag("archive-item:Old blog").assertDoesNotExist()
    }
}
