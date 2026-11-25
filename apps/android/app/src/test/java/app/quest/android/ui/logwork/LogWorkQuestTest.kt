package app.quest.android.ui.logwork

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.quest.android.LogWorkSamples
import app.quest.android.Samples
import app.quest.android.ui.theme.QuestTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.quest_mobile.LogWorkCheck

/** A session's quest follows its task: choosable only for a new task, else read-only or absent. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class LogWorkQuestTest {
    @get:Rule val compose = createComposeRule()

    private val form = LogWorkForm.of(LogWorkSamples.draft)

    private fun showLogWork(check: LogWorkCheck) {
        val state = LogWorkSheetState(LogWorkState(form, check, null), picker = null)
        compose.setContent { QuestTheme(dark = false) { LogWorkSheetContent(state, LogWorkActions.None) } }
    }

    @Test
    fun withoutTaskOrTitleThereIsNoQuestSection() {
        showLogWork(LogWorkSamples.check)

        compose.onNodeWithText("QUEST").assertDoesNotExist()
        compose.onNodeWithTag("form-quest-picker").assertDoesNotExist()
    }

    @Test
    fun aNewTasksQuestIsChosenFromTheActiveQuests() {
        showLogWork(LogWorkSamples.newTaskError)

        compose.onNodeWithTag("form-quest-picker").performClick()
        compose.onNodeWithTag("quest-picker-none").assertExists()
        compose.onNodeWithTag("quest-picker-option:${Samples.writing.name}").assertExists()
        compose.onNodeWithTag("form-quest").assertDoesNotExist()
    }

    @Test
    fun anExistingTaskShowsItsQuestReadOnly() {
        showLogWork(LogWorkSamples.existingTask)

        compose.onNodeWithTag("form-quest", useUnmergedTree = true).assertExists()
        compose.onNodeWithTag("form-quest-picker").assertDoesNotExist()
    }

    @Test
    fun anExistingTaskWithoutQuestShowsNoQuest() {
        showLogWork(LogWorkSamples.existingTask.copy(quest = null))

        compose.onNodeWithText("QUEST").assertDoesNotExist()
        compose.onNodeWithTag("form-quest", useUnmergedTree = true).assertDoesNotExist()
        compose.onNodeWithTag("form-quest-picker").assertDoesNotExist()
    }
}
