package app.quest.android.ui

/** What the outside asks the running app to show (widget deep link, launcher shortcut). */
sealed interface AppRequest {
    /** quest://task/<id>: the task's sheet over the current tab. */
    data class OpenTask(val taskId: String) : AppRequest

    /** "Log work" shortcut: the calendar with the Log work sheet. */
    data object LogWork : AppRequest
}
