package app.quest.android.ui

import android.os.Looper
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.ComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import java.time.Duration
import org.robolectric.Shadows.shadowOf

internal fun ComposeTestRule.liftAndDrag(from: String, to: String) {
    val root = onRoot()
    val origin = root.fetchSemanticsNode().boundsInRoot.topLeft
    val source = onNodeWithTag(from).fetchSemanticsNode().boundsInRoot
    val target = onNodeWithTag(to).fetchSemanticsNode().boundsInRoot
    root.performTouchInput { down(source.center - origin) }
    shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(650))
    mainClock.advanceTimeBy(650)
    root.performTouchInput { moveTo(Offset(target.center.x, target.top + target.height / 4) - origin, delayMillis = 16) }
    onNodeWithTag("drop-gap").assertIsDisplayed()
}
