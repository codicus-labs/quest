package app.quest.android.widget

import android.content.Context
import androidx.glance.GlanceId
import androidx.glance.action.ActionParameters
import androidx.glance.appwidget.action.ActionCallback
import app.quest.android.sync.saveForBackground

/** Checks or unchecks a task from the widget (the app may not be running). */
class ToggleTaskAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        val taskId = parameters[TaskIdKey] ?: return
        val host = context.applicationContext as WidgetHost
        if (!host.core.awaitOpen()) return
        host.core.perform { c, now -> c.toggleTask(taskId, now) }
        host.widgets.refresh()
        // The app may not be running, so the process can end right after: save, and sync soon.
        saveForBackground(context, host.core)
    }

    companion object {
        val TaskIdKey = ActionParameters.Key<String>("taskId")
    }
}
