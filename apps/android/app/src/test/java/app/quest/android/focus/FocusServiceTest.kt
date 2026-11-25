package app.quest.android.focus

import android.content.Intent
import android.os.Looper
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.Samples
import app.quest.android.core.CoreHolder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.android.controller.ServiceController
import org.robolectric.annotation.Config

/** The service's actions and when it stops, over a [FakeCore] that opens when the test says. */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], application = TestFocusApp::class)
class FocusServiceTest {
    private val app = RuntimeEnvironment.getApplication() as TestFocusApp
    private val core = FakeCore(running = Samples.running)

    /** The core opens (and re-reads) only when [settle] runs this dispatcher: a cold start. */
    private val io = StandardTestDispatcher()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private var starts = 0

    private fun setUpApp() {
        app.core = CoreHolder(scope, io, FakeClock()) { core }
        app.focusNotifier = FocusNotifier(app, app.core) { starts++ }.also { it.start(scope) }
        FocusNotifications.createChannel(app)
    }

    /** Lets the core open and every pending coroutine run. */
    private fun settle() {
        io.scheduler.advanceUntilIdle()
        shadowOf(Looper.getMainLooper()).idle()
    }

    private fun intent(action: String? = null, taskId: String? = null) =
        Intent(app, FocusService::class.java).setAction(action).putExtra(FocusService.EXTRA_TASK_ID, taskId)

    private fun service(): ServiceController<FocusService> = Robolectric.buildService(FocusService::class.java).create()

    private fun ServiceController<FocusService>.start(intent: Intent, startId: Int) = apply {
        get().onStartCommand(intent, 0, startId)
    }

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun pauseFromTheNotificationStopsTheServiceAndLeavesAPausedNotification() {
        setUpApp()
        settle()
        val controller = service().start(intent(FocusService.ACTION_PAUSE), 1)
        settle()
        val shadow = shadowOf(controller.get())

        assertEquals(listOf("stop"), core.calls)
        assertTrue(shadow.isForegroundStopped)
        assertFalse(shadow.notificationShouldRemoved)
        assertEquals(1, shadow.stopSelfId)
        val posted = shadowOf(app.getSystemService(android.app.NotificationManager::class.java)).getNotification(FocusNotifications.NOTIFICATION_ID)
        assertEquals(listOf("Resume", "Complete"), posted.actions.map { it.title.toString() })
    }

    @Test
    fun resumeRightAfterPauseOnAColdStartKeepsTheServiceRunning() {
        setUpApp()
        // Both taps arrive before the core is open.
        val controller = service().start(intent(FocusService.ACTION_PAUSE), 1).start(intent(FocusService.ACTION_RESUME, "t1"), 2)
        val shadow = shadowOf(controller.get())
        assertTrue("every start enters the foreground at once", shadow.lastForegroundNotification != null)

        settle()

        assertEquals(listOf("stop", "start t1"), core.calls)
        assertFalse(shadow.isStoppedBySelf)
        assertEquals(ShownNotification.Running(Samples.running), app.focusNotifier.shown)
        assertEquals(listOf("Pause", "Complete"), shadow.lastForegroundNotification.actions.map { it.title.toString() })
    }

    @Test
    fun completeFromTheNotificationCompletesAndRemovesIt() {
        setUpApp()
        settle()
        val controller = service().start(intent(FocusService.ACTION_COMPLETE, "t1"), 1)
        settle()
        val shadow = shadowOf(controller.get())

        assertEquals(listOf("complete t1"), core.calls)
        assertTrue(shadow.notificationShouldRemoved)
        assertTrue(shadow.isStoppedBySelf)
    }

    @Test
    fun aTimerEndedElsewhereWhileStartingEndsTheServiceOnlyAfterItIsInTheForeground() {
        setUpApp()
        settle()
        assertEquals(1, starts)
        // The notifier asked for the service; before it runs, another device stops the timer.
        core.running = null
        core.home = core.home.copy(dayLabel = "stopped elsewhere")
        app.core.perform { _, _ -> }
        settle()
        assertEquals(ShownNotification.None, app.focusNotifier.shown)

        val controller = service().start(intent(), 1)
        settle()
        val shadow = shadowOf(controller.get())

        // It entered the foreground (Android requires it), then removed the notification and stopped.
        assertEquals(FocusNotifications.NOTIFICATION_ID, shadow.lastForegroundNotificationId)
        assertTrue(shadow.notificationShouldRemoved)
        assertEquals(1, shadow.stopSelfId)
    }

    @Test
    fun anOlderStartDoesNotStopTheServiceANewerOneKeeps() {
        setUpApp()
        settle()
        val controller = service().start(intent(), 1).start(intent(), 2)
        settle()
        val shadow = shadowOf(controller.get())

        assertFalse(shadow.isStoppedBySelf)
        assertEquals(listOf("Pause", "Complete"), shadow.lastForegroundNotification.actions.map { it.title.toString() })
    }
}
