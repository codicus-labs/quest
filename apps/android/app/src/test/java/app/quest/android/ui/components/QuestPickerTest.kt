package app.quest.android.ui.components

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextInput
import app.quest.android.ui.theme.QuestTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.quest_mobile.QuestChip as QuestRef

/** The quest picker: opens from its chip, filters as you type, picks, offers "No quest" (last), says when nothing matches. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class QuestPickerTest {
    @get:Rule val compose = createComposeRule()

    private val quests = listOf("Tax return 2025", "Write the book", "Fitness", "Garden redesign", "Bike repair")
        .mapIndexed { i, name -> QuestRef("q$i", name, "#5c8fd9") }
    private val picks = mutableListOf<String?>()

    private fun showChip(initial: QuestRef?, noneLabel: String? = "No quest") {
        compose.setContent {
            QuestTheme(dark = false) {
                var quest by remember { mutableStateOf(initial) }
                QuestPickerChip(quest, quests, { picked -> picks += picked?.id; quest = picked }, "Quest", "chip", noneLabel = noneLabel)
            }
        }
        compose.onNodeWithTag("chip").performClick()
    }

    @Test
    fun theChipOpensTheListWithTheChosenQuestChecked() {
        showChip(quests[1])

        compose.onNodeWithTag("quest-picker-query").assertExists()
        compose.onNodeWithTag("quest-picker-none").assertIsNotSelected()
        compose.onNodeWithTag("quest-picker-option:Write the book").assertIsSelected()
        compose.onNodeWithTag("quest-picker-option:Fitness").assertIsNotSelected()
    }

    @Test
    fun typingFiltersByPartOfTheNameIgnoringCase() {
        showChip(null)

        compose.onNodeWithTag("quest-picker-query").performTextInput("  RE ")

        compose.onNodeWithTag("quest-picker-option:Tax return 2025").assertExists()
        compose.onNodeWithTag("quest-picker-option:Garden redesign").assertExists()
        compose.onNodeWithTag("quest-picker-option:Bike repair").assertExists()
        compose.onNodeWithTag("quest-picker-option:Write the book").assertDoesNotExist()
        compose.onNodeWithTag("quest-picker-option:Fitness").assertDoesNotExist()
        compose.onNodeWithTag("quest-picker-none").assertDoesNotExist()
    }

    @Test
    fun pickingAQuestChoosesItAndCloses() {
        showChip(null)

        compose.onNodeWithTag("quest-picker-option:Fitness").performClick()

        assertEquals(listOf<String?>("q2"), picks)
        compose.onNodeWithTag("quest-picker-query").assertDoesNotExist()
    }

    @Test
    fun noQuestClearsTheChoice() {
        showChip(quests[0])

        compose.onNodeWithTag("quest-picker-none").performClick()

        assertEquals(listOf<String?>(null), picks)
    }

    @Test
    fun noQuestComesLastAndIsFoundBySearchToo() {
        showChip(quests[0])

        compose.onNodeWithTag("quest-picker-query").performTextInput("no q")
        compose.onNodeWithTag("quest-picker-option:Tax return 2025").assertDoesNotExist()
        compose.onNodeWithTag("quest-picker-query").performImeAction()

        assertEquals(listOf<String?>(null), picks)
    }

    @Test
    fun withoutANoneLabelThereIsNoNoQuestEntry() {
        showChip(quests[0], noneLabel = null)

        compose.onNodeWithTag("quest-picker-none").assertDoesNotExist()
        compose.onNodeWithTag("quest-picker-option:Tax return 2025").assertExists()
    }

    @Test
    fun nothingMatchingShowsTheEmptyState() {
        showChip(null)

        compose.onNodeWithTag("quest-picker-query").performTextInput("xyz")

        compose.onNodeWithTag("quest-picker-empty").assertExists()
        compose.onNodeWithTag("quest-picker-list").assertDoesNotExist()
    }

    @Test
    fun theKeyboardsDoneTakesTheFirstMatch() {
        showChip(null)

        compose.onNodeWithTag("quest-picker-query").performTextInput("bo")
        compose.onNodeWithTag("quest-picker-query").performImeAction()

        assertEquals(listOf<String?>("q1"), picks)
    }
}
