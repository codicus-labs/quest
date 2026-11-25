package app.quest.android.ui.quests

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.Dp
import app.quest.android.QuestSamples
import app.quest.android.ui.theme.QuestTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Renaming in place keeps the type and the place of what is renamed: nothing jumps or resizes. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class QuestRenameLayoutTest {
    @get:Rule val compose = createComposeRule()

    private var inputs by mutableStateOf(QuestDetailInputs(questId = "q1"))

    private fun showPage() = compose.setContent {
        QuestTheme(dark = false) {
            QuestDetailScreen(QuestDetailUiState.Ready(QuestSamples.detail, inputs), QuestDetailActions.None)
        }
    }

    /** The node's text style (size, weight, line height, tracking, font), first baseline, and where the text starts and how wide it is. */
    private fun look(tag: String): Triple<List<Any?>, Float, List<Dp>> {
        val node = compose.onNodeWithTag(tag, useUnmergedTree = true)
        val layouts = mutableListOf<TextLayoutResult>()
        node.fetchSemanticsNode().config[SemanticsActions.GetTextLayoutResult].action!!.invoke(layouts)
        val layout = layouts.single()
        val style = layout.layoutInput.style
        // Both text and field draw their text from the node's top-left corner.
        val bounds = node.getUnclippedBoundsInRoot()
        return Triple(
            listOf(style.fontSize, style.fontWeight, style.lineHeight, style.letterSpacing, style.fontFamily),
            layout.getLineBaseline(0),
            listOf(bounds.left, bounds.top, bounds.right - bounds.left),
        )
    }

    @Test
    fun theQuestNameFieldHasTheHeadingsTypeAndPlace() {
        showPage()
        val heading = look("quest-title")

        inputs = inputs.copy(title = QuestSamples.detail.title)
        compose.waitForIdle()

        compose.onNodeWithTag("quest-rename").assertIsFocused()
        assertEquals(heading, look("quest-rename"))
    }
}
