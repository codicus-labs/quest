package app.quest.android.ui.lists

import android.os.Looper
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import app.quest.android.ListSamples
import app.quest.android.ui.theme.QuestTheme
import java.time.Duration
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import uniffi.quest_mobile.PlanTaskList

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class SortableTaskListTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun stationaryFingerAtEdgeScrollsAndCarriesTheGap() {
        val row = ListSamples.todayOpen.first()
        val entries = (0..60).map {
            SortItem.Row("row-$it", PlanTaskList.Today, row.copy(id = "$it", title = "Task $it"))
        }
        val state = LazyListState()
        var anchor: String? = null
        compose.setContent {
            QuestTheme(dark = false) {
                SortableTaskList(state, entries, RowActions({}, {}, {}, {}), { _, _, before -> anchor = before },
                    Modifier.fillMaxSize(),
                ) { placed, draw ->
                    LazyColumn(Modifier.fillMaxSize(), state = state) { sortItems(placed, draw) }
                }
            }
        }

        val root = compose.onRoot()
        val origin = root.fetchSemanticsNode().boundsInRoot.topLeft
        val start = compose.onNodeWithTag("row:Task 2").fetchSemanticsNode().boundsInRoot.center - origin
        root.performTouchInput { down(start) }
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(650))
        compose.mainClock.advanceTimeBy(650)
        compose.mainClock.autoAdvance = false
        root.performTouchInput { moveTo(Offset(start.x, height.toFloat() - 5f)) }
        compose.mainClock.advanceTimeByFrame()
        compose.onNodeWithTag("drop-gap").assertIsDisplayed()
        val (before, lastBefore) = compose.runOnIdle {
            state.firstVisibleItemIndex to state.layoutInfo.visibleItemsInfo.last().index
        }

        compose.mainClock.advanceTimeBy(400)

        val after = compose.runOnIdle { state.firstVisibleItemIndex }
        assertTrue("Expected edge scrolling without pointer movement: $before → $after", after > before)
        compose.onNodeWithTag("drop-gap").assertIsDisplayed()
        root.performTouchInput { up() }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        assertTrue("Drop anchor $anchor did not follow scrolling ($before → $after; last visible was $lastBefore)",
            (anchor?.toIntOrNull() ?: 0) > lastBefore)
    }
}
