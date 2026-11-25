package app.quest.android.focus

import android.app.Notification
import app.quest.android.Samples
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], application = TestFocusApp::class)
class FocusNotificationsTest {
    private val app = RuntimeEnvironment.getApplication()

    @Test
    fun aRunningTimerShowsOnlyTheTimerOnALockedScreen() {
        val notification = FocusNotifications.build(app, runningContent(Samples.running))
        val locked = notification.publicVersion

        assertEquals(Notification.VISIBILITY_PRIVATE, notification.visibility)
        assertEquals("Outline chapter 3", notification.extras.getString(Notification.EXTRA_TITLE))
        assertEquals("Focus", locked.extras.getString(Notification.EXTRA_TITLE))
        assertNull(locked.extras.getCharSequence(Notification.EXTRA_TEXT))
        assertEquals(true, locked.extras.getBoolean(Notification.EXTRA_SHOW_CHRONOMETER))
        assertEquals(Samples.running.chronometerBaseMs, locked.`when`)
        assertNull(locked.actions)
    }

    @Test
    fun aPausedTimerSaysOnlyThatOnALockedScreen() {
        val locked = FocusNotifications.build(app, pausedContent(Samples.running)).publicVersion

        assertEquals("Focus paused", locked.extras.getString(Notification.EXTRA_TITLE))
        assertNull(locked.extras.getCharSequence(Notification.EXTRA_TEXT))
        assertNull(locked.actions)
    }

    @Test
    fun aTimerInAnotherWorkspaceSaysWhichOne() {
        val elsewhere = Samples.running.copy(workspaceName = "Default")
        val running = FocusNotifications.build(app, runningContent(elsewhere))
        val paused = FocusNotifications.build(app, pausedContent(elsewhere))

        assertEquals("Write the book · in Default", running.extras.getCharSequence(Notification.EXTRA_TEXT).toString())
        assertEquals("Paused · Write the book · in Default", paused.extras.getCharSequence(Notification.EXTRA_TEXT).toString())
        assertNull(running.publicVersion.extras.getCharSequence(Notification.EXTRA_TEXT))
    }

    @Test
    fun thePlaceholderHasNothingPrivateAndIsPublic() {
        assertEquals(Notification.VISIBILITY_PUBLIC, FocusNotifications.placeholder(app).visibility)
    }
}
