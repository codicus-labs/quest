package app.quest.android.widget

import android.content.Context
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver

/** The widget's broadcast receiver (the day change goes to [DayChangeReceiver], which is not exported). */
class QuestWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = QuestWidget()

    override fun onDisabled(context: Context) {
        super.onDisabled(context)
        (context.applicationContext as WidgetHost).widgets.cancelDayChange()
    }
}
