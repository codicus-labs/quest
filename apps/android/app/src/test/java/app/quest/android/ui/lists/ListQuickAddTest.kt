package app.quest.android.ui.lists

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import app.quest.android.ListSamples
import app.quest.android.Samples
import app.quest.android.ui.theme.QuestTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.quest_mobile.QuickFields

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ListQuickAddTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun focusShowsDefaultsAndSendingKeepsFocusAndResetsThem() {
        val defaults = QuickAddFields(QuickFields(null, "2026-11-25", null), null, "Nov 25", null)
        var fields by mutableStateOf(defaults)
        var draft by mutableStateOf("")
        var submitted = ""
        val actions = ListActions(
            RowActions({}, {}, {}, {}),
            editDraft = { draft = it },
            add = { submitted = draft; draft = ""; fields = defaults },
            undo = {},
            dismissToast = {},
            quick = QuickAddActions(
                suggestions = { query -> if (query == "wri") listOf(Samples.writing) else emptyList() },
                setQuest = { text, id ->
                    draft = text.trimEnd()
                    fields = defaults.copy(fields = defaults.fields.copy(questId = id), quest = Samples.writing)
                },
            ),
        )
        compose.setContent { QuestTheme(false) { ListQuickAdd(draft, fields, actions) } }

        val input = compose.onNodeWithTag("list-add").assertIsFocused()
        compose.onNodeWithTag("field-plan").assertIsDisplayed()
        input.performTextInput("Review #wri")
        compose.onNodeWithText(Samples.writing.name).performClick()
        compose.onNodeWithText("${Samples.writing.name} ▾").assertIsDisplayed()
        compose.onNodeWithTag("list-add-send").performClick()
        input.assertIsFocused()
        compose.onNodeWithText("No quest ▾").assertIsDisplayed()
        compose.runOnIdle { assertEquals("Review", submitted) }
    }

    @Test
    fun playAndStopKeepTheSameBounds() {
        val running = ListSamples.runningRow
        var row by mutableStateOf(running.copy(timerRunning = false, timer = null))
        val actions = RowActions({}, {}, { row = running }, { row = row.copy(timerRunning = false) })
        compose.setContent { QuestTheme(false) { Column { TaskRow(row, actions) } } }

        val play = compose.onNodeWithTag("timer-start:${running.title}")
        val originalBounds = play.fetchSemanticsNode().boundsInRoot
        play.performClick()
        val stop = compose.onNodeWithTag("timer-stop:${running.title}")
        assertEquals(originalBounds, stop.fetchSemanticsNode().boundsInRoot)
        compose.onNodeWithText(running.timer!!).assertDoesNotExist()
        stop.performClick()
        assertEquals(originalBounds, play.fetchSemanticsNode().boundsInRoot)
    }
}
