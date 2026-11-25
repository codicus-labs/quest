package app.quest.android.focus

import android.app.ForegroundServiceStartNotAllowedException
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.util.Log
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uniffi.quest_mobile.RunningSession

private const val TAG = "FocusNotifier"

/**
 * Keeps the focus service and notification in line with the core's running timer (one per
 * process, in `QuestApplication`). It reads `running_session` after every core change and acts on
 * [decideService]. The notification's buttons come back here through [FocusService].
 *
 * It never stops the service itself (stopping one whose start is still pending crashes the app):
 * it asks the running service to [ServiceHandle.settle], and a service still starting settles on
 * its own once it is in the foreground. Runs on the main thread; the core's calls only touch memory.
 */
class FocusNotifier(
    private val context: Context,
    private val core: CoreHolder,
    private val startForegroundService: () -> Unit = { context.startForegroundService(Intent(context, FocusService::class.java)) },
) {
    /** The running service, as the notifier sees it. */
    interface ServiceHandle {
        /** Show [shown] now: stay in the foreground while it is [ShownNotification.Running], else stop. */
        fun settle()
    }

    /** What the notification shows; only the main thread changes it. */
    var shown: ShownNotification = ShownNotification.Unknown
        private set

    private var service: ServiceHandle? = null
    private val notifications = context.getSystemService(NotificationManager::class.java)

    fun start(scope: CoroutineScope) {
        scope.launch {
            core.state.collect { state -> if (state is CoreState.Ready) act(decideService(shown, reading() ?: return@collect)) }
        }
    }

    fun attach(handle: ServiceHandle) {
        service = handle
    }

    fun detach(handle: ServiceHandle) {
        if (service === handle) service = null
    }

    /** The app came to the front: a timer shown without its service gets it now. */
    fun appResumed() = act(shown.retryService())

    /**
     * Posts the shown notification again: without notification permission Android drops it, and
     * granting later does not bring it back. Called when the permission is granted.
     */
    fun repost() {
        shown.content()?.let(::post)
    }

    /** The service's notification right now: the shown content, else the running timer's. */
    fun currentContent(): FocusNotificationContent? =
        shown.content() ?: readRunning()?.session?.let { session ->
            shown = ShownNotification.Running(session)
            runningContent(session)
        }

    /** The service could not enter the foreground: the timer shows as a plain notification. */
    fun serviceRefused() {
        val running = shown as? ShownNotification.Running ?: return
        shown = ShownNotification.Fallback(running.session)
        post(runningContent(running.session))
    }

    /** Pause from the notification: stops the timer and leaves a paused notification. */
    fun pause() {
        val session = when (val current = shown) {
            is ShownNotification.Running -> current.session
            is ShownNotification.Fallback -> current.session
            else -> readRunning()?.session
        } ?: return
        // Before the core changes: the change must not look like "ended elsewhere" (which removes it).
        shown = ShownNotification.Paused(session)
        report(core.perform { c, now -> c.stopTimer(now) })
    }

    /** Resume from the notification; the service shows the running timer if it started. */
    fun resume(taskId: String) {
        report(core.perform { c, now -> c.startTimer(taskId, now) })
        shown = readRunning()?.session?.let { ShownNotification.Running(it) } ?: ShownNotification.None
    }

    /** Complete from the notification: completes the task (stopping its timer); the notification goes. */
    fun complete(taskId: String) {
        shown = ShownNotification.None
        report(core.perform { c, now -> c.completeTask(taskId, now) })
    }

    private fun act(decision: ServiceDecision) {
        shown = shown.after(decision)
        when (decision) {
            ServiceDecision.Nothing -> Unit
            is ServiceDecision.Start -> startService(decision.session)
            is ServiceDecision.Update -> post(runningContent(decision.session))
            ServiceDecision.Stop -> service?.settle() ?: notifications.cancel(FocusNotifications.NOTIFICATION_ID)
        }
    }

    /**
     * Starts the service. From the background (a timer synced in while the app is closed) Android
     * 12+ may refuse; then a plain notification shows the timer until the app comes to the front.
     */
    private fun startService(session: RunningSession) {
        try {
            startForegroundService()
        } catch (e: ForegroundServiceStartNotAllowedException) {
            Log.w(TAG, "Foreground service not allowed now; showing a plain notification", e)
            shown = ShownNotification.Fallback(session)
            post(runningContent(session))
        }
    }

    /** The core's timer now; null if the read failed (then nothing changes). */
    private fun reading(): TimerReading? {
        val running = readRunning() ?: return null
        val paused = shown as? ShownNotification.Paused
        if (running.session != null || paused == null) return TimerReading(running.session)
        // The task may be in another workspace than the shown one (the timer is one per person).
        val taskId = paused.session.taskId ?: return TimerReading(null, pausedTaskOpen = false)
        val open = core.read { c, _ -> c.isTaskOpen(taskId) } as? Outcome.Done ?: return null
        return TimerReading(null, pausedTaskOpen = open.value)
    }

    private class Running(val session: RunningSession?)

    private fun readRunning(): Running? = when (val read = core.read { c, now -> c.runningSession(now) }) {
        is Outcome.Done -> Running(read.value)
        else -> null
    }

    private fun post(content: FocusNotificationContent) {
        notifications.notify(FocusNotifications.NOTIFICATION_ID, FocusNotifications.build(context, content))
    }

    private fun report(outcome: Outcome<*>) {
        // Only the kind: the core's message can name tasks or the server.
        if (outcome !is Outcome.Done) Log.w(TAG, "Notification action failed: ${outcome::class.simpleName}")
    }
}
