package app.quest.android.ui.calendar

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.quest.android.CalendarSamples
import app.quest.android.Samples
import app.quest.android.ui.theme.QuestTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.quest_mobile.EntryCheck
import uniffi.quest_mobile.LogWorkCheck

/** A session's quest follows its task: choosable only for a new task, else read-only or absent. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class LogWorkQuestTest {
    @get:Rule val compose = createComposeRule()

    private val form = LogWorkForm.of(CalendarSamples.logWorkDraft)

    private fun showLogWork(check: LogWorkCheck) {
        val state = CalendarUiState.Ready(CalendarSamples.page(1), LogWorkState(form, check, null), entry = null, picker = null)
        compose.setContent { QuestTheme(dark = false) { CalendarSheetContent(state, CalendarSheetActions.None) } }
    }

    private fun showEntry(check: EntryCheck) {
        val entry = EntryState(EntryForm.of(CalendarSamples.entryDraft), check, null)
        val state = CalendarUiState.Ready(CalendarSamples.page(1), logWork = null, entry = entry, picker = null)
        compose.setContent { QuestTheme(dark = false) { CalendarSheetContent(state, CalendarSheetActions.None) } }
    }

    @Test
    fun withoutTaskOrTitleThereIsNoQuestSection() {
        showLogWork(CalendarSamples.logWorkCheck)

        compose.onNodeWithText("QUEST").assertDoesNotExist()
        compose.onNodeWithTag("form-quest-picker").assertDoesNotExist()
    }

    @Test
    fun aNewTasksQuestIsChosenFromTheActiveQuests() {
        showLogWork(CalendarSamples.logWorkNewTaskError)

        compose.onNodeWithTag("form-quest-picker").performClick()
        compose.onNodeWithTag("quest-picker-none").assertExists()
        compose.onNodeWithTag("quest-picker-option:${Samples.writing.name}").assertExists()
        compose.onNodeWithTag("form-quest").assertDoesNotExist()
    }

    @Test
    fun anExistingTaskShowsItsQuestReadOnly() {
        showLogWork(CalendarSamples.logWorkExistingTask)

        compose.onNodeWithTag("form-quest", useUnmergedTree = true).assertExists()
        compose.onNodeWithTag("form-quest-picker").assertDoesNotExist()
    }

    @Test
    fun anExistingTaskWithoutQuestShowsNoQuest() {
        showLogWork(CalendarSamples.logWorkExistingTask.copy(quest = null))

        compose.onNodeWithText("QUEST").assertDoesNotExist()
        compose.onNodeWithTag("form-quest", useUnmergedTree = true).assertDoesNotExist()
        compose.onNodeWithTag("form-quest-picker").assertDoesNotExist()
    }

    @Test
    fun theEntryEditorShowsItsTasksQuestOnlyWhenThereIsOne() {
        showEntry(CalendarSamples.entryCheck.copy(taskTitle = null, quest = null))

        compose.onNodeWithTag("form-quest", useUnmergedTree = true).assertDoesNotExist()
    }
}
