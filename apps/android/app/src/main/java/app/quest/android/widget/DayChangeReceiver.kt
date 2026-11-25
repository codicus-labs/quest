package app.quest.android.widget

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/**
 * Redraws the widget for a new day: the day-change alarm, or the clock or time zone changed (which
 * moves the day under it). Not exported; the system's time broadcasts still reach it.
 */
class DayChangeReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val host = context.applicationContext as WidgetHost
        val pending = goAsync()
        CoroutineScope(SupervisorJob() + Dispatchers.Default).launch {
            try {
                host.widgets.refresh()
            } finally {
                pending.finish()
            }
        }
    }
}
