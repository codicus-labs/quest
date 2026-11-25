package app.quest.android.ui.components

import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** The in-place rename field's keys. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class InlineTitleFieldTest {
    @get:Rule val compose = createComposeRule()

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun enterSavesInsteadOfBreakingTheLine() {
        val edits = mutableListOf<String>()
        var commits = 0
        compose.setContent {
            QuestTheme(dark = false) {
                InlineTitleField("Write the book", { edits += it }, { commits++ }, QuestType.heading, "Name", Modifier.testTag("field"))
            }
        }
        compose.onNodeWithTag("field").assertIsFocused()

        compose.onNodeWithTag("field").performKeyInput { pressKey(Key.Enter) }
        compose.waitForIdle()

        assertEquals(1, commits)
        assertEquals(emptyList<String>(), edits)
    }
}
