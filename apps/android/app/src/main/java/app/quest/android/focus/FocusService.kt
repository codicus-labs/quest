package app.quest.android.focus

import android.app.ForegroundServiceStartNotAllowedException
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.util.Log
import app.quest.android.core.CoreHolder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

private const val TAG = "FocusService"

/** What the focus service needs from the application (`QuestApplication`; a test app in tests). */
interface FocusHost {
    val core: CoreHolder
    val focusNotifier: FocusNotifier
}

/**
 * The foreground service while a focus timer runs: it keeps the process (and so sync and the
 * ticker) alive when the app is closed, and owns the ongoing notification. [FocusNotifier] decides
 * what shows; the notification's buttons arrive here as actions.
 *
 * Every start enters the foreground at once (Android requires it), then handles its action once
 * the core is open, then settles on what the notifier shows. Only the newest start may stop the
 * service (`stopSelf(startId)`), so a Resume right after a Pause is never lost.
 *
 * Type `specialUse` (Android 14+): a user-started stopwatch that must stay in sync with other
 * devices fits none of the specific types (see `docs/mobile.md`, Phase 2).
 */
class FocusService :
    Service(),
    FocusNotifier.ServiceHandle {
    private val host get() = application as FocusHost
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val notifications by lazy { getSystemService(NotificationManager::class.java) }

    private var latestStartId = 0

    /** Starts whose action has not run yet; the last one to finish settles. */
    private var pending = 0

    override fun onCreate() {
        super.onCreate()
        host.core.setServiceRunning(true)
        host.focusNotifier.attach(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        latestStartId = startId
        if (!goForeground(host.focusNotifier.currentContent())) {
            host.focusNotifier.serviceRefused()
            stopSelf(startId)
            return START_NOT_STICKY
        }
        val action = intent?.action
        val taskId = intent?.getStringExtra(EXTRA_TASK_ID)
        pending++
        scope.launch {
            if (host.core.awaitOpen()) handle(action, taskId)
            pending--
            settle()
        }
        // After a process kill the next app start (or sync) restarts it; a sticky restart would have no session yet.
        return START_NOT_STICKY
    }

    private fun handle(action: String?, taskId: String?) {
        val notifier = host.focusNotifier
        when (action) {
            ACTION_PAUSE -> notifier.pause()
            ACTION_RESUME -> taskId?.let(notifier::resume)
            ACTION_COMPLETE -> taskId?.let(notifier::complete)
            // Started by the notifier: show what it shows.
            else -> Unit
        }
    }

    override fun settle() {
        if (pending > 0) return
        val shown = host.focusNotifier.shown
        val content = shown.content()
        when {
            shown is ShownNotification.Running -> goForeground(content)
            content != null -> {
                // Paused (or shown without the service): the notification stays, the service goes.
                stopForeground(STOP_FOREGROUND_DETACH)
                notifications.notify(FocusNotifications.NOTIFICATION_ID, FocusNotifications.build(this, content))
                stopSelf(latestStartId)
            }
            else -> {
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf(latestStartId)
            }
        }
    }

    /** Returns false when Android refused the foreground start. */
    private fun goForeground(content: FocusNotificationContent?): Boolean {
        val notification = content?.let { FocusNotifications.build(this, it) } ?: FocusNotifications.placeholder(this)
        return try {
            startForeground(FocusNotifications.NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
            true
        } catch (e: ForegroundServiceStartNotAllowedException) {
            Log.w(TAG, "Foreground start not allowed", e)
            false
        }
    }

    override fun onDestroy() {
        scope.cancel()
        host.focusNotifier.detach(this)
        host.core.setServiceRunning(false)
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        const val ACTION_PAUSE = "app.quest.android.focus.PAUSE"
        const val ACTION_RESUME = "app.quest.android.focus.RESUME"
        const val ACTION_COMPLETE = "app.quest.android.focus.COMPLETE"
        const val EXTRA_TASK_ID = "task_id"
    }
}
