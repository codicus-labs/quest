package app.quest.android.ui.components

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.Dp
import app.quest.android.QuestSamples
import app.quest.android.ui.review.ReviewActions
import app.quest.android.ui.review.ReviewScreen
import app.quest.android.ui.review.ReviewUiState
import app.quest.android.ui.theme.QuestTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** A period's arrows stay put while one steps through periods with shorter and longer labels. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class PeriodArrowsTest {
    @get:Rule val compose = createComposeRule()

    private fun left(tag: String): Dp = compose.onNodeWithTag(tag).getUnclippedBoundsInRoot().left

    /** Where the two arrows are for each label. */
    private fun arrowsFor(labels: List<String>, show: (String) -> Unit, previous: String, next: String): Set<Pair<Dp, Dp>> =
        labels.map { label ->
            show(label)
            compose.waitForIdle()
            left(previous) to left(next)
        }.toSet()

    @Test
    fun reviewArrowsStayPut() {
        var label by mutableStateOf("")
        compose.setContent {
            QuestTheme(dark = false) {
                ReviewScreen(ReviewUiState.Ready(QuestSamples.lastWeek.copy(weekLabel = label)), ReviewActions.None)
            }
        }
        val seen = arrowsFor(listOf("May 4 – 10", "Sep 28 – Oct 4", "Nov 16 – 22"), { label = it }, "review-previous", "review-next")
        assertEquals("one place for the arrows: $seen", 1, seen.size)
    }
}
