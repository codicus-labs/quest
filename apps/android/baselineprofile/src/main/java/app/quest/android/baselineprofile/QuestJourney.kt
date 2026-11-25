package app.quest.android.baselineprofile

import androidx.benchmark.macro.MacrobenchmarkScope
import androidx.test.uiautomator.By
import androidx.test.uiautomator.Direction
import androidx.test.uiautomator.Until

const val PACKAGE = "app.quest.android"
private const val WAIT_MS = 5_000L

/** Compose test tags are resource ids in the app (`testTagsAsResourceId`). */
private fun MacrobenchmarkScope.waitFor(tag: String) =
    device.wait(Until.findObject(By.res(tag)), WAIT_MS) ?: error("\"$tag\" did not show within $WAIT_MS ms.")

/** Waits until Home shows its first content. */
fun MacrobenchmarkScope.awaitHome() {
    waitFor("tab-home")
}

/** What a user does most: scroll Home, open the plan and scroll it, open the calendar and turn a page. */
fun MacrobenchmarkScope.everydayJourney() {
    awaitHome()
    device.findObject(By.scrollable(true))?.apply {
        setGestureMargin(device.displayWidth / 5)
        fling(Direction.DOWN)
        fling(Direction.UP)
    }
    waitFor("tab-plan").click()
    device.waitForIdle()
    device.findObject(By.scrollable(true))?.apply {
        setGestureMargin(device.displayWidth / 5)
        fling(Direction.DOWN)
        fling(Direction.UP)
    }
    waitFor("tab-calendar").click()
    device.waitForIdle()
    device.swipe(device.displayWidth * 4 / 5, device.displayHeight / 2, device.displayWidth / 5, device.displayHeight / 2, 20)
    device.waitForIdle()
    device.swipe(device.displayWidth / 5, device.displayHeight / 2, device.displayWidth * 4 / 5, device.displayHeight / 2, 20)
    device.waitForIdle()
    waitFor("tab-home").click()
    device.waitForIdle()
}
