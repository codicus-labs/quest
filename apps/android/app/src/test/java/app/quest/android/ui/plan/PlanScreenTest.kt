package app.quest.android.ui.plan

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToKey
import androidx.compose.ui.test.performTouchInput
import app.quest.android.FakeCore
import app.quest.android.PlanSamples
import app.quest.android.ui.liftAndDrag
import app.quest.android.ui.lists.RowActions
import app.quest.android.ui.openedHolder
import app.quest.android.ui.theme.QuestTheme
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.quest_mobile.PlanTaskList

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class PlanScreenTest {
    @get:Rule val compose = createComposeRule()

    private fun show(actions: PlanActions) = compose.setContent {
        QuestTheme(dark = false) { PlanScreen(PlanUiState.Ready(PlanSamples.page), actions) }
    }

    @Test
    fun weekDayJumpsToTheDayHeadingAndItsPlusUsesThatDate() {
        val added = mutableListOf<String>()
        show(PlanActions(RowActions({}, {}, {}, {}), added::add, { _, _, _ -> }, {}, {}))

        compose.onNodeWithTag("plan-week-${PlanSamples.friday}").performClick()
        val heading = compose.onNodeWithTag("plan-day-${PlanSamples.friday}").assertIsDisplayed()
        val list = compose.onNodeWithTag("plan-sections")
        assertEquals(list.fetchSemanticsNode().boundsInRoot.top, heading.fetchSemanticsNode().boundsInRoot.top, 0.5f)

        compose.onNodeWithTag("plan-add-${PlanSamples.friday}").performClick()
        assertEquals(listOf(PlanSamples.friday), added)
    }

    @Test
    fun dayHeadingsUseRelativeLabelsAndAlignAcrossDigitCounts() {
        show(PlanActions(RowActions({}, {}, {}, {}), {}, { _, _, _ -> }, {}, {}))

        compose.onNodeWithText("Today").assertIsDisplayed()
        val list = compose.onNodeWithTag("plan-sections")
        list.performScrollToKey(PlanSamples.thursday)
        compose.onNodeWithText("Tomorrow").assertIsDisplayed()
        list.performScrollToKey("2026-11-30")
        fun labelLeft(date: String, label: String) =
            compose.onNode(hasText(label) and hasAnyAncestor(hasTestTag("plan-day-$date")))
                .assertIsDisplayed().fetchSemanticsNode().boundsInRoot.left

        assertEquals(labelLeft("2026-11-30", "Monday"), labelLeft("2026-12-01", "Tuesday"), 0.5f)
    }

    @Test
    fun unplannedTaskCanDropIntoALaterMonth() {
        val placements = mutableListOf<Triple<String, PlanTaskList, String?>>()
        show(PlanActions(RowActions({}, {}, {}, {}), {}, { id, list, anchor -> placements += Triple(id, list, anchor) }, {}, {}))

        val list = compose.onNodeWithTag("plan-sections")
        list.performScrollToKey("not-planned")
        compose.onNodeWithTag("not-planned").performClick()
        list.performScrollToKey("unplanned-p5")
        compose.liftAndDrag("row:Choose a cover photo", "row:Edit chapter 3")
        compose.onRoot().performTouchInput { up() }
        compose.waitForIdle()

        assertEquals(listOf(Triple("p5", PlanTaskList.Day("2027-01-07"), "p4")), placements)
    }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test
    fun crossDayDropCallsPlaceTaskWithTheRowUnderTheGapAsAnchor() = runTest(UnconfinedTestDispatcher()) {
        val core = FakeCore()
        val vm = PlanViewModel(openedHolder(core, backgroundScope, UnconfinedTestDispatcher(testScheduler)))
        show(PlanActions(RowActions({}, {}, {}, {}), {}, vm::placeTask, {}, {}))

        compose.onNodeWithTag("plan-week-${PlanSamples.thursday}").performClick()
        compose.liftAndDrag("row:Water the plants", "row:Interval run")
        compose.onRoot().performTouchInput { up() }
        compose.waitForIdle()

        assertEquals(listOf(Triple("p3", PlanTaskList.Day(PlanSamples.thursday), "p2")), core.placements)
    }
}
