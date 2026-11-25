package app.quest.android.focus

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import app.quest.android.MainActivity
import app.quest.android.R
import app.quest.android.ui.components.questColor
import androidx.compose.ui.graphics.toArgb

/** The focus notification on Android: its channel, builder and the intents behind its buttons. */
object FocusNotifications {
    const val CHANNEL_ID = "focus"
    const val NOTIFICATION_ID = 1

    /** The deep link that opens Focus (notification tap, `adb shell am start -d quest://focus`). */
    val FOCUS_URI: Uri = Uri.parse("quest://focus")

    /** Quiet (no sound, no heads-up): it only shows the running timer. */
    fun createChannel(context: Context) {
        val channel = NotificationChannel(CHANNEL_ID, context.getString(R.string.notification_channel_focus), NotificationManager.IMPORTANCE_LOW).apply {
            description = context.getString(R.string.notification_channel_focus_description)
            setShowBadge(false)
        }
        context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }

    fun build(context: Context, content: FocusNotificationContent): Notification {
        val questName = content.questName ?: context.getString(R.string.no_quest)
        val quest = content.workspaceName?.let { context.getString(R.string.notification_in_workspace, questName, it) } ?: questName
        val text = if (content.running) quest else context.getString(R.string.notification_paused, quest)
        val builder = Notification.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_focus)
            .setContentTitle(content.title)
            .setContentText(text)
            .setCategory(Notification.CATEGORY_STOPWATCH)
            .setOngoing(content.ongoing)
            .setOnlyAlertOnce(true)
            .setForegroundServiceBehavior(Notification.FOREGROUND_SERVICE_IMMEDIATE)
            .setContentIntent(openFocus(context))
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setPublicVersion(lockScreenVersion(context, content))
        content.questColorHex?.let { builder.setColor(questColor(it).toArgb()) }
        showTime(builder, content.chronometerBaseMs)
        content.actions.forEach { builder.addAction(action(context, it, content.taskId)) }
        return builder.build()
    }

    /** On a locked screen: the timer without task, quest or buttons. */
    private fun lockScreenVersion(context: Context, content: FocusNotificationContent): Notification {
        val title = if (content.running) R.string.focus else R.string.notification_paused_public
        val builder = Notification.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_focus)
            .setContentTitle(context.getString(title))
            .setCategory(Notification.CATEGORY_STOPWATCH)
        showTime(builder, content.chronometerBaseMs)
        return builder.build()
    }

    private fun showTime(builder: Notification.Builder, chronometerBaseMs: Long?) {
        if (chronometerBaseMs == null) {
            builder.setShowWhen(false)
            return
        }
        // The system counts; no per-second updates from the app.
        builder.setUsesChronometer(true).setWhen(chronometerBaseMs).setShowWhen(true)
    }

    /** A notification for the moment between starting the service and knowing the session; nothing private in it. */
    fun placeholder(context: Context): Notification =
        Notification.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_focus)
            .setContentTitle(context.getString(R.string.focus))
            .setContentIntent(openFocus(context))
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .build()

    private fun openFocus(context: Context): PendingIntent {
        val intent = Intent(Intent.ACTION_VIEW, FOCUS_URI, context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        return PendingIntent.getActivity(context, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    /**
     * Buttons start the service as a foreground service: tapping a notification may start one
     * from the background, and Resume needs it running again.
     */
    private fun action(context: Context, action: NotificationAction, taskId: String?): Notification.Action {
        val (label, serviceAction) = when (action) {
            NotificationAction.Pause -> R.string.timer_pause to FocusService.ACTION_PAUSE
            NotificationAction.Resume -> R.string.focus_resume to FocusService.ACTION_RESUME
            NotificationAction.Complete -> R.string.task_complete to FocusService.ACTION_COMPLETE
        }
        val intent = Intent(context, FocusService::class.java).setAction(serviceAction).putExtra(FocusService.EXTRA_TASK_ID, taskId)
        val pending = PendingIntent.getForegroundService(context, action.ordinal + 1, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        return Notification.Action.Builder(null, context.getString(label), pending).build()
    }
}
