package app.quest.android.ui.plan

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTextInput
import app.quest.android.PlanSamples
import app.quest.android.settings.HomePrefs
import app.quest.android.ui.theme.QuestTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** The plan's cards without dragging: accessibility actions and the day's add field. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class PlanScreenTest {
    @get:Rule val compose = createComposeRule()

    private val calls = mutableListOf<String>()
    private var inputs by mutableStateOf(PlanInputs())

    private val actions = PlanActions(
        {}, {}, {}, {}, {}, {}, {}, {},
        editDraft = { day, text -> inputs = inputs.withDraft(day, text) },
        addTask = {}, setAddPosition = {}, setQuestFilter = {}, setShowOther = {},
        visibleRange = { _, _ -> }, jumpTo = {}, scrolled = {},
        moveTask = { id, day, before -> calls += "move $id $day $before" },
        nudgeTask = { id, up -> calls += "nudge $id ${if (up) "up" else "down"}" },
        shiftTask = { id, later -> calls += "shift $id ${if (later) "later" else "earlier"}" },
        openMoveTo = { id, _ -> calls += "move-to $id" },
        moveTo = {}, closeMoveTo = {}, edgeSpeed = { _, _, _ -> 0f },
    )

    private fun show() = compose.setContent {
        QuestTheme(dark = false) {
            PlanScreen(PlanUiState.Ready(PlanSamples.plan, HomePrefs(), inputs, moveSheet = null, scroll = ScrollRequest("2026-11-24", 0, animate = false)), actions)
        }
    }

    private fun SemanticsNodeInteraction.performCustomAction(label: String) {
        val action = fetchSemanticsNode().config.getOrNull(SemanticsActions.CustomActions)?.firstOrNull { it.label == label }
            ?: error("no action $label")
        compose.runOnIdle { action.action() }
    }

    @Test
    fun aCardMovesWithoutDraggingThroughItsAccessibilityActions() {
        show()
        val card = compose.onNodeWithTag("task:Interval run")

        listOf("Move to…", "Move up", "Move down", "Move to previous day", "Move to next day").forEach { card.performCustomAction(it) }

        assertEquals(listOf("move-to p2", "nudge p2 up", "nudge p2 down", "shift p2 earlier", "shift p2 later"), calls)
    }

    @Test
    fun theAddFieldKeepsWhatIsTypedAndEmptiesWhenTheDraftIsAdded() {
        show()
        val field = compose.onNodeWithTag("plan-add:2026-11-24")

        field.performTextInput("Buy")
        field.performTextInput(" stamps")
        compose.waitForIdle()
        assertEquals("Buy stamps", inputs.draft("2026-11-24"))

        inputs = inputs.withDraft("2026-11-24", "")
        field.assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")))
    }
}
