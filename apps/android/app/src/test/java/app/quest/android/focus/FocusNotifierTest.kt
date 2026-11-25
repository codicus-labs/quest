package app.quest.android.focus

import android.app.ForegroundServiceStartNotAllowedException
import android.app.NotificationManager
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.Samples
import app.quest.android.core.CoreHolder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/** The notifier over a [FakeCore]: when it starts the service, what it posts, and what it never does. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], application = TestFocusApp::class)
class FocusNotifierTest {
    private val app = RuntimeEnvironment.getApplication()
    private val core = FakeCore()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private val holder = CoreHolder(scope, Dispatchers.Unconfined, FakeClock()) { core }
    private val notifications = shadowOf(app.getSystemService(NotificationManager::class.java))

    private var starts = 0
    private var refuseStart = false
    private val notifier = FocusNotifier(app, holder) {
        starts++
        if (refuseStart) throw ForegroundServiceStartNotAllowedException("app in background")
    }

    private var changes = 0

    /** Another core change (the fake's data otherwise looks the same, and the state would not change). */
    private fun coreChanged() {
        core.home = core.home.copy(dayLabel = "change ${++changes}")
        holder.perform { _, _ -> }
    }

    private class CountingService : FocusNotifier.ServiceHandle {
        var settles = 0

        override fun settle() {
            settles++
        }
    }

    @Before
    fun setUp() = FocusNotifications.createChannel(app)

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun aRunningTimerStartsTheServiceOnce() {
        core.running = Samples.running
        notifier.start(scope)
        coreChanged()

        assertEquals(1, starts)
        assertEquals(ShownNotification.Running(Samples.running), notifier.shown)
    }

    @Test
    fun aRefusedStartShowsAPlainNotificationAndRetriesWhenTheAppComesBack() {
        core.running = Samples.running
        refuseStart = true
        notifier.start(scope)

        assertEquals(ShownNotification.Fallback(Samples.running), notifier.shown)
        assertEquals(1, notifications.size())
        coreChanged()
        assertEquals(1, starts)

        refuseStart = false
        notifier.appResumed()

        assertEquals(2, starts)
        assertEquals(ShownNotification.Running(Samples.running), notifier.shown)
    }

    @Test
    fun anEndedTimerAsksTheServiceToSettleInsteadOfStoppingIt() {
        core.running = Samples.running
        notifier.start(scope)
        val service = CountingService()
        notifier.attach(service)

        core.running = null
        coreChanged()

        assertEquals(1, service.settles)
        assertEquals(ShownNotification.None, notifier.shown)
    }

    @Test
    fun withoutAServiceAnEndedTimersPlainNotificationGoes() {
        core.running = Samples.running
        refuseStart = true
        notifier.start(scope)

        core.running = null
        coreChanged()

        assertEquals(0, notifications.size())
        assertEquals(ShownNotification.None, notifier.shown)
    }

    @Test
    fun aNotificationLeftFromBeforeTheProcessStartedGoesWhenNoTimerRuns() {
        app.getSystemService(NotificationManager::class.java)
            .notify(FocusNotifications.NOTIFICATION_ID, FocusNotifications.build(app, pausedContent(Samples.running)))

        notifier.start(scope)

        assertEquals(0, notifications.size())
        assertEquals(ShownNotification.None, notifier.shown)
    }

    @Test
    fun aPausedNotificationStaysWhileItsTaskIsOpenAndGoesWhenItIsDone() {
        core.running = Samples.running
        notifier.start(scope)
        notifier.pause()
        assertEquals(listOf("stop"), core.calls)
        assertEquals(ShownNotification.Paused(Samples.running), notifier.shown)

        coreChanged()
        assertEquals(ShownNotification.Paused(Samples.running), notifier.shown)

        // Completed on another device.
        core.closedTasks += "t1"
        coreChanged()
        assertEquals(ShownNotification.None, notifier.shown)
        assertEquals(0, notifications.size())
    }

    @Test
    fun comingBackToTheAppDoesNotBringBackADismissedNotification() {
        core.running = Samples.running
        notifier.start(scope)
        notifier.pause()
        app.getSystemService(NotificationManager::class.java).cancelAll()

        notifier.appResumed()

        assertEquals(0, notifications.size())
    }

    @Test
    fun resumeAndCompleteFromTheNotificationReachTheCore() {
        core.running = Samples.running
        notifier.start(scope)
        notifier.pause()

        notifier.resume("t1")
        assertEquals(ShownNotification.Running(Samples.running), notifier.shown)

        notifier.complete("t1")
        assertEquals(listOf("stop", "start t1", "complete t1"), core.calls)
        assertEquals(ShownNotification.None, notifier.shown)
        assertEquals(1, starts)
    }
}
