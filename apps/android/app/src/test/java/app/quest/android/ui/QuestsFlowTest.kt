package app.quest.android.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeFocusPreferences
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

/** The Quests tab with its editor sheet and the archive, driven through the whole app. */
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

    private fun assertFigures(active: String, completed: String, inProgress: String, completedCount: String) {
        compose.onNodeWithTag("quests-tab-active").assertTextEquals(active)
        compose.onNodeWithTag("quests-tab-completed").assertTextEquals(completed)
        compose.onNodeWithTag("quests-in-progress", useUnmergedTree = true).assertTextEquals(inProgress)
        compose.onNodeWithTag("quests-completed", useUnmergedTree = true).assertTextEquals(completedCount)
    }

    @Test
    fun aQuestGoesFromActiveToCompletedAndBackWithItsHours() {
        showQuests()
        assertFigures("Active · 2", "Completed · 1", "02", "01")

        tap("quest-card:Write the book")
        tap("quest-editor-status")
        assertFigures("Active · 1", "Completed · 2", "01", "02")
        compose.onNodeWithTag("quest-card:Write the book").assertDoesNotExist()
        compose.onNodeWithText("Completed quest “Write the book”").assertIsDisplayed()

        tap("quests-tab-completed")
        compose.onNodeWithTag("quest-week:Write the book", useUnmergedTree = true).assertTextEquals("4.5h")
        compose.onNodeWithTag("quest-total:Write the book", useUnmergedTree = true).assertTextEquals("62h total")
        tap("quest-card:Write the book")
        compose.onNodeWithTag("quest-editor-status").assertTextEquals("Reopen quest")
        tap("quest-editor-status")

        assertFigures("Active · 2", "Completed · 1", "02", "01")
        tap("quests-tab-active")
        compose.onNodeWithTag("quest-week:Write the book", useUnmergedTree = true).assertTextEquals("4.5h")
        assertEquals(listOf("toggle-quest q1", "toggle-quest q1"), core.calls)
    }

    @Test
    fun undoInTheSnackbarTakesACompletionBack() {
        showQuests()
        tap("quest-card:Write the book")
        tap("quest-editor-status")

        compose.onNodeWithTag("snackbar-action").performClick()
        compose.waitForIdle()

        assertFigures("Active · 2", "Completed · 1", "02", "01")
        compose.onNodeWithTag("quest-card:Write the book").assertExists()
        assertEquals(listOf("toggle-quest q1", "undo"), core.calls)
    }

    @Test
    fun closingTheEditorKeepsTheTabAndTheScroll() {
        val many = List(12) { QuestSamples.doneCard.copy(id = "c$it", title = "Done quest $it") }
        core.quests = QuestSamples.listed(QuestSamples.quests, QuestSamples.quests.active, many)
        core.questDetails["c10"] = QuestSamples.toggled(QuestSamples.detail).copy(id = "c10", title = "Done quest 10")
        val viewModels = showQuests()
        tap("quests-tab-completed")
        compose.onNodeWithTag("quests-list").performScrollToNode(hasTestTag("quest-card:Done quest 10"))
        compose.waitForIdle()

        compose.onNodeWithTag("quest-card:Done quest 10").performClick()
        compose.onNodeWithTag("quest-editor-title").assertExists()
        compose.runOnIdle { viewModels.questEditor.close() }
        compose.waitForIdle()

        compose.onNodeWithTag("quest-editor-title").assertDoesNotExist()
        compose.onNodeWithTag("quest-card:Done quest 10").assertIsDisplayed()
        // Still scrolled down: the figures above the tabs are out of view (a lazy grid drops them).
        compose.onNodeWithTag("quests-in-progress", useUnmergedTree = true).assertDoesNotExist()
        compose.onNodeWithTag("quests-list").performScrollToNode(hasTestTag("quests-tab-completed"))
        compose.onNodeWithTag("quests-tab-completed").assertIsSelected()
    }

    @Test
    fun newQuestCreatesTheQuestFromItsTitleOnly() {
        core.questDetails["q-new"] = QuestSamples.detail.copy(id = "q-new", title = "Learn piano")
        showQuests()

        tap("quests-new")
        compose.onNode(hasText("New quest") and isHeading()).assertExists()
        compose.onNodeWithTag("quest-editor-title").performTextInput("Learn piano")
        tap("quest-editor-create")

        assertEquals(listOf("create-quest Learn piano|null"), core.calls)
        compose.onNode(hasText("Edit quest") and isHeading()).assertExists()
        compose.onNodeWithTag("quest-editor-status").assertExists()
    }

    @Test
    fun anArchivedQuestIsInTheArchiveUntilUnarchived() {
        showQuests()
        tap("quests-archive")
        compose.onNodeWithText("Archived quests", ignoreCase = true).assertIsDisplayed()

        tap("archive-unarchive:Old blog")

        assertEquals(listOf("unarchive-quest q8"), core.calls)
        compose.onNodeWithTag("archive-item:Old blog").assertDoesNotExist()
        compose.onNodeWithTag("back").performClick()
        compose.waitForIdle()
        compose.onNodeWithTag("quests-list").performScrollToNode(hasTestTag("quest-card:Old blog"))
        compose.onNodeWithTag("quest-card:Old blog").assertIsDisplayed()
        compose.onNodeWithTag("quests-list").performScrollToNode(hasTestTag("quests-tab-active"))
        assertFigures("Active · 3", "Completed · 1", "03", "01")
    }

    @Test
    fun openQuestPageLeavesTheSheetForThePage() {
        showQuests()
        tap("quest-card:Write the book")

        tap("quest-editor-page")

        compose.onNodeWithTag("quest-editor-title").assertDoesNotExist()
        compose.onNodeWithTag("quest-detail").assertIsDisplayed()
    }
}
