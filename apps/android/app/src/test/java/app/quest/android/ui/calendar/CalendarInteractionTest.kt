package app.quest.android.ui.calendar

import androidx.compose.foundation.ScrollState
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import app.quest.android.CalendarSamples
import app.quest.android.ui.theme.QuestTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** The calendar's taps and drags reach the right actions, through the real screen. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class CalendarInteractionTest {
    @get:Rule val compose = createComposeRule()

    private val calls = mutableListOf<String>()

    private val dropMinutes = mutableListOf<Float>()

    private fun showCalendar(edgeSpeed: Float = 0f, scroll: ScrollState = ScrollState(0)) {
        val grid = CalendarGridActions(
            preview = { _, _, _ -> CalendarSamples.placement },
            drop = { drag, day, minute ->
                calls += "drop ${drag.kind} ${drag.blockId} $day"
                dropMinutes += minute
            },
            tapBlock = { calls += "tap ${it.id}" },
            edgeSpeed = { _, _, _ -> edgeSpeed },
        )
        val actions = CalendarActions({}, {}, {}, { calls += "log-work" }, {}, {}, grid)
        val state = CalendarUiState.Ready(CalendarSamples.page(1), logWork = null, entry = null, picker = null)
        compose.setContent { QuestTheme(dark = false) { CalendarScreen(state, actions, scroll = scroll) } }
    }

    @Test
    fun theFabOpensLogWork() {
        showCalendar()

        compose.onNodeWithTag("calendar-log-work").performClick()

        assertEquals(listOf("log-work"), calls)
    }

    @Test
    fun aBlockOpensByTapAndByItsAccessibilityAction() {
        showCalendar()

        compose.onNodeWithTag("grid-block:t4", useUnmergedTree = true).performTouchInput { click(center) }
        compose.onNodeWithTag("grid-block:t4", useUnmergedTree = true).performSemanticsAction(SemanticsActions.OnClick)

        assertEquals(listOf("tap t4", "tap t4"), calls)
    }

    @Test
    fun aBlockMovesAndEndsInQuarterHoursThroughItsAccessibilityActions() {
        showCalendar()
        val block = compose.onNodeWithTag("grid-block:t4", useUnmergedTree = true)
        val labels = block.fetchSemanticsNode().config[SemanticsActions.CustomActions].map { it.label }
        assertEquals(listOf("Move 15 minutes earlier", "Move 15 minutes later", "End 15 minutes earlier", "End 15 minutes later"), labels)

        for (label in labels) {
            block.fetchSemanticsNode().config[SemanticsActions.CustomActions].single { it.label == label }.action()
        }

        val start = 11 * 60f
        val end = 12 * 60f + 30
        assertEquals(List(2) { "drop MOVE t4 2026-11-23" } + List(2) { "drop END_EDGE t4 2026-11-23" }, calls)
        assertEquals(listOf(start - 15, start + 15, end - 15, end + 15), dropMinutes)
    }

    @Test
    fun aLongPressDragMovesTheBlockOnRelease() {
        showCalendar()

        compose.onNodeWithTag("grid-block:t4", useUnmergedTree = true).performTouchInput {
            down(center)
            advanceEventTime(viewConfiguration.longPressTimeoutMillis + 100)
            moveBy(Offset(0f, 60f))
            moveBy(Offset(0f, 60f))
            up()
        }
        compose.waitForIdle()

        assertEquals(listOf("drop MOVE t4 2026-11-23"), calls)
    }

    @Test
    fun anEdgePillResizesWithoutALongPress() {
        showCalendar()

        compose.onNodeWithTag("grid-handle-end:t4", useUnmergedTree = true).performTouchInput {
            down(center)
            moveBy(Offset(0f, 40f))
            moveBy(Offset(0f, 40f))
            up()
        }
        compose.waitForIdle()

        assertEquals(listOf("drop END_EDGE t4 2026-11-23"), calls)
    }

    @Test
    fun anEdgePillTakesAFullTouchTargetAroundItsSmallPill() {
        showCalendar()

        // 10 dp below the 4 dp pill: outside the drawn pill and the block, inside the handle's touch zone.
        compose.onNodeWithTag("grid-handle-end:t4", useUnmergedTree = true).performTouchInput {
            down(center + Offset(0f, 10f))
            moveBy(Offset(0f, 40f))
            moveBy(Offset(0f, 40f))
            up()
        }
        compose.waitForIdle()

        assertEquals(listOf("drop END_EDGE t4 2026-11-23"), calls)
    }

    @Test
    fun aDragFarBesideTheEdgeScrollsInsteadOfResizing() {
        showCalendar()

        // 20 dp below the block's end: beyond the handle's outward reach.
        compose.onNodeWithTag("grid-handle-end:t4", useUnmergedTree = true).performTouchInput {
            down(center + Offset(0f, 20f))
            moveBy(Offset(0f, 40f))
            moveBy(Offset(0f, 40f))
            up()
        }
        compose.waitForIdle()

        assertEquals(emptyList<String>(), calls)
    }

    @Test
    fun theMiddleOfAShortBlockIsNoHandle() {
        showCalendar()

        // t3 is 30 minutes (26 dp): its middle lies within 24 dp of both edges, but past a third of the block.
        compose.onNodeWithTag("grid-block:t3", useUnmergedTree = true).performTouchInput {
            down(center)
            moveBy(Offset(0f, 40f))
            moveBy(Offset(0f, 40f))
            up()
        }
        compose.waitForIdle()

        assertEquals(emptyList<String>(), calls)
    }

    @Test
    fun aDragRestingAtTheEdgeScrollsTheGridAndDropsWhereTheFingerIsNow() {
        val scroll = ScrollState(0)
        // Upwards: the page opens at 09:00, close to the bottom of a phone's scroll range.
        showCalendar(edgeSpeed = -400f, scroll = scroll)
        compose.waitForIdle()
        val before = scroll.value

        val block = compose.onNodeWithTag("grid-block:t4", useUnmergedTree = true)
        block.performTouchInput {
            down(center)
            advanceEventTime(viewConfiguration.longPressTimeoutMillis + 100)
            moveBy(Offset(0f, 30f))
        }
        // The scroll loop asks for a frame every frame while the finger rests: drive the clock by hand.
        compose.mainClock.autoAdvance = false
        compose.mainClock.advanceTimeBy(500)
        val scrolled = before - scroll.value
        block.performTouchInput { up() }
        compose.mainClock.advanceTimeBy(100)

        assertTrue("the grid scrolled ($scrolled px)", scrolled > 100)
        assertEquals(listOf("drop MOVE t4 2026-11-23"), calls)
        // Without the scroll the drop would land 30 px (at 52 dp an hour, mdpi) below the grab.
        assertTrue("the drop follows the scroll", dropMinutes.single() < grabMinuteOfT4() + 30f / 52f * 60f - 60f)
    }

    private fun grabMinuteOfT4(): Float {
        val block = CalendarSamples.page(1).days.first().grid.blocks.first { it.id == "t4" }
        return (block.startMinute + block.endMinute).toFloat() / 2f
    }

    @Test
    fun aLongPressOnEmptySpaceDrawsANewSpan() {
        showCalendar()

        // 40 % down the visible grid is empty whether or not it has scrolled to the morning yet.
        compose.onNodeWithTag("calendar-grid").performTouchInput {
            down(Offset(width * 0.6f, height * 0.4f))
            advanceEventTime(viewConfiguration.longPressTimeoutMillis + 100)
            moveBy(Offset(0f, 60f))
            up()
        }
        compose.waitForIdle()

        assertEquals(listOf("drop NEW_SPAN null 2026-11-23"), calls)
    }

    @Test
    fun aCancelledDragDropsNothingAndTheNextTapStillWorks() {
        showCalendar()

        compose.onNodeWithTag("grid-block:t4", useUnmergedTree = true).performTouchInput {
            down(center)
            advanceEventTime(viewConfiguration.longPressTimeoutMillis + 100)
            moveBy(Offset(0f, 60f))
            cancel()
        }
        compose.waitForIdle()
        compose.onNodeWithTag("calendar-ghost").assertDoesNotExist()

        compose.onNodeWithTag("grid-block:t4", useUnmergedTree = true).performTouchInput { click(center) }
        assertEquals(listOf("tap t4"), calls)
    }
}
