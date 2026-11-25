package app.quest.android.focus

import uniffi.quest_mobile.RunningSession

/** What the focus notification shows right now. */
sealed interface ShownNotification {
    /** Just after the process started: a notification from before (paused, or a killed service's) may still be up. */
    data object Unknown : ShownNotification

    data object None : ShownNotification

    /** The ongoing notification of the foreground service. */
    data class Running(val session: RunningSession) : ShownNotification

    /**
     * A timer runs but Android refused the foreground service (started from the background): a
     * plain notification shows it until the app comes to the front, which starts the service.
     */
    data class Fallback(val session: RunningSession) : ShownNotification

    /**
     * Paused from the notification: the service stopped and left this notification (Resume,
     * Complete). It stays until the user acts on it, dismisses it, a timer starts, or its task is
     * no longer open (completed or deleted anywhere).
     */
    data class Paused(val session: RunningSession) : ShownNotification
}

/**
 * What the core says: the running timer, and whether the paused notification's task is still
 * open (read only while one is [ShownNotification.Paused]).
 */
data class TimerReading(val running: RunningSession?, val pausedTaskOpen: Boolean = true)

/** What to do about the service and notification when the core changed. */
sealed interface ServiceDecision {
    data object Nothing : ServiceDecision

    /** A timer runs and no service shows it: start the foreground service. */
    data class Start(val session: RunningSession) : ServiceDecision

    /** The running timer changed (another task, renamed, other quest): refresh the notification. */
    data class Update(val session: RunningSession) : ServiceDecision

    /** Nothing should show any more: stop the service (if any) and remove the notification. */
    data object Stop : ServiceDecision
}

/**
 * The service follows the core's running timer only: it starts when one runs (also when sync
 * brought it), and stops when none does (also when another device ended it). A notification left
 * by pausing from the notification stays while its task is open and no timer runs.
 */
fun decideService(shown: ShownNotification, reading: TimerReading): ServiceDecision {
    val running = reading.running ?: return when (shown) {
        ShownNotification.None -> ServiceDecision.Nothing
        is ShownNotification.Paused -> if (reading.pausedTaskOpen) ServiceDecision.Nothing else ServiceDecision.Stop
        ShownNotification.Unknown, is ShownNotification.Running, is ShownNotification.Fallback -> ServiceDecision.Stop
    }
    return when (shown) {
        is ShownNotification.Running -> if (shown.session == running) ServiceDecision.Nothing else ServiceDecision.Update(running)
        is ShownNotification.Fallback -> if (shown.session == running) ServiceDecision.Nothing else ServiceDecision.Update(running)
        ShownNotification.Unknown, ShownNotification.None, is ShownNotification.Paused -> ServiceDecision.Start(running)
    }
}

/** When the app comes to the front: a timer shown without its service gets the service now. */
fun ShownNotification.retryService(): ServiceDecision =
    if (this is ShownNotification.Fallback) ServiceDecision.Start(session) else ServiceDecision.Nothing

/** What the notification shows after [decision] (a refused [ServiceDecision.Start] then becomes [ShownNotification.Fallback]). */
fun ShownNotification.after(decision: ServiceDecision): ShownNotification = when (decision) {
    ServiceDecision.Nothing -> this
    is ServiceDecision.Start -> ShownNotification.Running(decision.session)
    is ServiceDecision.Update -> if (this is ShownNotification.Fallback) ShownNotification.Fallback(decision.session) else ShownNotification.Running(decision.session)
    ServiceDecision.Stop -> ShownNotification.None
}

/** A button on the focus notification. */
enum class NotificationAction { Pause, Resume, Complete }

/** The focus notification's content, independent of Android's builder (so it is testable). */
data class FocusNotificationContent(
    val taskId: String,
    val title: String,
    /** The quest's name; null without a quest. */
    val questName: String?,
    val questColorHex: String?,
    /** Set when the timer runs in another workspace than the shown one. */
    val workspaceName: String?,
    /** Where the chronometer counts from (epoch ms); null while paused (no chronometer). */
    val chronometerBaseMs: Long?,
    val running: Boolean,
    /** Swipe-dismissible only while paused; the running one belongs to the service. */
    val ongoing: Boolean,
    val actions: List<NotificationAction>,
)

/** Running: chronometer from the core's base (no per-second updates), Pause and Complete. */
fun runningContent(session: RunningSession) = FocusNotificationContent(
    taskId = session.taskId,
    title = session.title,
    questName = session.questName,
    questColorHex = session.questColorHex,
    workspaceName = session.workspaceName,
    chronometerBaseMs = session.chronometerBaseMs,
    running = true,
    ongoing = true,
    actions = listOf(NotificationAction.Pause, NotificationAction.Complete),
)

/** Paused from the notification: no chronometer, Resume and Complete, dismissible. */
fun pausedContent(session: RunningSession) = FocusNotificationContent(
    taskId = session.taskId,
    title = session.title,
    questName = session.questName,
    questColorHex = session.questColorHex,
    workspaceName = session.workspaceName,
    chronometerBaseMs = null,
    running = false,
    ongoing = false,
    actions = listOf(NotificationAction.Resume, NotificationAction.Complete),
)

fun ShownNotification.content(): FocusNotificationContent? = when (this) {
    ShownNotification.Unknown, ShownNotification.None -> null
    is ShownNotification.Running -> runningContent(session)
    is ShownNotification.Fallback -> runningContent(session)
    is ShownNotification.Paused -> pausedContent(session)
}
