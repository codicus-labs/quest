package app.quest.android.widget

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.updateAll
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.conflate
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import uniffi.quest_mobile.HomeData
import uniffi.quest_mobile.HomeQuery

/** What the widget shows: the core's Home without filter, or why not. */
sealed interface WidgetData {
    data object Opening : WidgetData

    data class Ready(val home: HomeData) : WidgetData

    data class Failed(val message: String) : WidgetData
}

/** What the widget pieces need from the application (`QuestApplication`). */
interface WidgetHost {
    val core: CoreHolder
    val widgets: WidgetUpdater
}

/** Widget redraws after a burst of core changes are at most this often. */
private const val CHANGE_THROTTLE_MS = 2_000L

/** The widget's Home: no filter and no pick, like a fresh Home. */
private val widgetQuery = HomeQuery(questFilter = null, questPick = null)

/** What the widget shows that changes with time alone: the day and its hours (a running timer adds to them). */
private data class ShownTimes(val dayIso: String, val todayHours: String, val weekHours: String)

private fun shownTimes(home: HomeData) = ShownTimes(home.dayIso, home.todayHours, home.weekHours)

/**
 * Keeps the home-screen widgets current (one per process): re-reads Home and redraws them on core
 * changes (throttled, ticks do not count), when a tick changes the day or the hours shown, and from a
 * day-change alarm at the core's `next_day_ms` while the app sleeps. Does nothing while no widget is placed.
 *
 * A running timer: the core ticks while the app is visible or the focus service runs (which it does
 * while a timer runs), and the hours text ("3.6h") changes at most every six minutes; so the widget
 * follows the timer with no alarm of its own and redraws only when its text changes.
 */
class WidgetUpdater(private val context: Context, private val core: CoreHolder) {
    private val _data = MutableStateFlow<WidgetData>(WidgetData.Opening)
    val data: StateFlow<WidgetData> = _data.asStateFlow()

    private val refreshing = Mutex()

    fun start(scope: CoroutineScope) {
        scope.launch {
            core.changes.conflate().collect {
                refresh()
                delay(CHANGE_THROTTLE_MS)
            }
        }
        scope.launch {
            core.state
                .map { (it as? CoreState.Ready)?.home?.let(::shownTimes) }
                .filterNotNull()
                .distinctUntilChanged()
                .drop(1)
                .collect { refresh() }
        }
    }

    /** Re-reads Home and redraws every placed widget; plans the next day-change alarm. */
    suspend fun refresh() = refreshing.withLock {
        if (!hasWidgets()) return@withLock
        reload()
        QuestWidget().updateAll(context)
    }

    /** Fresh data for a widget about to draw (also in a cold process). */
    suspend fun reload() {
        if (!core.awaitOpen()) {
            _data.value = (core.state.value as? CoreState.Failed)?.let { WidgetData.Failed(it.message) } ?: WidgetData.Opening
            return
        }
        _data.value = when (val home = core.read { c, now -> c.home(now, widgetQuery) }) {
            is Outcome.Done -> WidgetData.Ready(home.value)
            is Outcome.Failed -> WidgetData.Failed(home.message)
            Outcome.NotReady -> WidgetData.Opening
        }
        scheduleDayChange()
    }

    private suspend fun hasWidgets(): Boolean = withContext(Dispatchers.Default) {
        GlanceAppWidgetManager(context).getGlanceIds(QuestWidget::class.java).isNotEmpty()
    }

    /** An inexact alarm at the next local day (the core's time); the receiver redraws then. */
    private fun scheduleDayChange() {
        val nextDay = (core.read { c, now -> c.nextDayMs(now) } as? Outcome.Done)?.value ?: return
        val alarms = context.getSystemService(AlarmManager::class.java) ?: return
        alarms.set(AlarmManager.RTC, nextDay, dayChangeIntent(context))
    }

    /** No widget left: the day-change alarm goes. */
    fun cancelDayChange() {
        context.getSystemService(AlarmManager::class.java)?.cancel(dayChangeIntent(context))
    }

    companion object {
        private fun dayChangeIntent(context: Context): PendingIntent = PendingIntent.getBroadcast(
            context,
            0,
            Intent(context, DayChangeReceiver::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }
}
