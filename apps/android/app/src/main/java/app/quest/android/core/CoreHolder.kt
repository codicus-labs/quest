package app.quest.android.core

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.quest_mobile.Account
import uniffi.quest_mobile.ArchiveData
import uniffi.quest_mobile.QuestDetail
import uniffi.quest_mobile.QuestsData
import uniffi.quest_mobile.ReviewData
import uniffi.quest_mobile.CalendarData
import uniffi.quest_mobile.CalendarQuery
import uniffi.quest_mobile.DayGrid
import uniffi.quest_mobile.HomeData
import uniffi.quest_mobile.HomeQuery
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.Moment
import uniffi.quest_mobile.PlanData
import uniffi.quest_mobile.PlanWindow
import uniffi.quest_mobile.SettingsData
import uniffi.quest_mobile.SyncOutcome
import uniffi.quest_mobile.SyncState
import uniffi.quest_mobile.WorkspacesData

/** What the screens render, re-read from the core after every change. */
sealed interface CoreState {
    data object Opening : CoreState

    data class Ready(
        val home: HomeData,
        /** Today's day grid; re-read on every tick, so its now-line moves. */
        val todayGrid: DayGrid,
        val sync: SyncState,
        val account: Account?,
        val settings: SettingsData,
        /** The switcher, "Manage workspaces" and a timer running in another workspace. */
        val workspaces: WorkspacesData,
        /** The plan's days for the current [PlanQuery]; null while no plan is shown. */
        val plan: PlanData? = null,
        /** The calendar page for the current [CalendarQuery]; null while no calendar is shown. */
        val calendar: CalendarData? = null,
        /** The Quests tab; null while it is not shown. */
        val quests: QuestsData? = null,
        /** The open quest page; null while none is open. */
        val questPage: QuestPage? = null,
        /** The review of the asked week; null while the review is not shown. */
        val review: ReviewData? = null,
        /** The archive; null while it is not shown. */
        val archive: ArchiveData? = null,
    ) : CoreState

    data class Failed(val message: String) : CoreState
}

/** The open quest page as read: the quest, or gone (deleted here or elsewhere). */
sealed interface QuestPage {
    val questId: String

    data class Shown(val detail: QuestDetail) : QuestPage {
        override val questId get() = detail.id
    }

    data class Gone(override val questId: String) : QuestPage
}

/** The review's week: any ISO day in it, null for this week. */
data class ReviewQuery(val week: String?)

/** What the plan shows: the built [window] of days, QUEST WORK filtered by [questFilter]. */
data class PlanQuery(val window: PlanWindow, val questFilter: String?)

/** The result of one action on the core. */
sealed interface Outcome<out T> {
    data class Done<T>(val value: T) : Outcome<T>

    data class Failed(val message: String) : Outcome<Nothing>

    /** The core is still opening (or failed to open); nothing happened. */
    data object NotReady : Outcome<Nothing>
}

/** What to tell the user about an action that did not happen; null when it did. */
fun Outcome<*>.failureMessage(): String? = when (this) {
    is Outcome.Done -> null
    is Outcome.Failed -> message
    Outcome.NotReady -> "Quest is still opening. Try again in a moment."
}

/** How long to wait before ticking again after a failed tick. */
private const val TICK_RETRY_MS = 60_000L

/** [CoreHolder.syncUntilIdle] waits on the core in slices this long, so a stopped caller ends after at most one. */
internal const val SYNC_SLICE_MS = 1_000L

/**
 * What to tell the user about [this] core failure. [MobileException]s carry the core's own message;
 * anything else (a UniFFI `InternalException` from a panic, a JNA error) gets [fallback] plus detail.
 */
private fun Exception.userMessage(fallback: String): String = when (this) {
    is MobileException -> message ?: fallback
    else -> "$fallback (${this::class.simpleName}: $message)"
}

/**
 * The one core per process (held by `QuestApplication`): opens it off the main thread, publishes
 * [state] whenever the core reports a change, and ticks it while the app is in the foreground or the
 * focus service runs.
 * The core decides when to wake (next second, minute or midnight); this class only schedules.
 *
 * Every change (core callback, action, tick) only marks the state stale; a single reader coroutine
 * on [io] re-reads the core and is the only writer of [state]. So the core's callback never calls
 * back into the core on its worker thread, and an older read cannot overwrite a newer one.
 */
class CoreHolder(
    private val scope: CoroutineScope,
    private val io: CoroutineDispatcher,
    private val clock: Clock,
    private val open: (onChange: () -> Unit) -> QuestCoreApi,
) {
    private val _state = MutableStateFlow<CoreState>(CoreState.Opening)
    val state: StateFlow<CoreState> = _state.asStateFlow()

    private val _notices = MutableSharedFlow<String>(extraBufferCapacity = 8, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    /** Problems from background work (ticks, refreshes) that no screen asked for. */
    val notices: SharedFlow<String> = _notices.asSharedFlow()

    private val _changes = MutableSharedFlow<Unit>(extraBufferCapacity = 1, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    /** The core's data changed (an action here, or sync brought one); unlike [state], ticks do not count. */
    val changes: SharedFlow<Unit> = _changes.asSharedFlow()

    /**
     * Whether AI entries show (dictate shortcut, share target, quick add's mic, search's action): the signed-in
     * account turned AI features on and its server has an AI provider. False for the guest. Nothing until the core
     * is open, so the entries keep what they showed instead of flickering off on every start.
     */
    val aiFeatures: Flow<Boolean> =
        state.filterIsInstance<CoreState.Ready>().map { it.account?.aiFeatures == true }.distinctUntilChanged()

    @Volatile private var core: QuestCoreApi? = null

    /** Completes with the core once opening finished, or with null if it failed. */
    private val opened = CompletableDeferred<QuestCoreApi?>()
    private var ticker: Job? = null

    /** Home's filter and quest pick: UI state the core reads to build [CoreState.Ready.home]. */
    @Volatile private var homeQuery = HomeQuery(questFilter = null, questPick = null)

    /** The plan's query while the plan is on screen; null skips reading it. */
    @Volatile private var planQuery: PlanQuery? = null

    /** The calendar's page while the calendar is on screen; null skips reading it. */
    @Volatile private var calendarQuery: CalendarQuery? = null

    @Volatile private var questsShown = false

    /** The quest page's quest while it is open. */
    @Volatile private var questPageId: String? = null

    @Volatile private var reviewQuery: ReviewQuery? = null

    @Volatile private var archiveShown = false

    /** Whether the last read built the plan; only the reader coroutine touches it. */
    private var planRead = false

    /** "The core changed since the last read"; conflated, so a burst of changes means one re-read. */
    private val stale = Channel<Unit>(Channel.CONFLATED)

    /**
     * "Data changed, the next wake time may have too" (a timer started, sync brought one):
     * wakes the tick loop early, so a running timer ticks every second right away.
     */
    private val reschedule = Channel<Unit>(Channel.CONFLATED)

    init {
        scope.launch(io) {
            val core = openCore() ?: return@launch
            for (signal in stale) publish(core)
        }
    }

    private fun openCore(): QuestCoreApi? {
        val core = try {
            open(::coreChanged)
        } catch (e: Exception) {
            _state.value = CoreState.Failed(e.userMessage("Could not open Quest's data. Restart the app."))
            opened.complete(null)
            return null
        }
        this.core = core
        opened.complete(core)
        tickOnce(core)
        return core
    }

    /** Safe from any thread, including the core's worker inside its own callback. */
    private fun markStale() {
        stale.trySend(Unit)
    }

    /** The core's data changed (callback or action): re-read, and re-plan the next tick. */
    private fun coreChanged() {
        markStale()
        reschedule.trySend(Unit)
        _changes.tryEmit(Unit)
    }

    private var appVisible = false
    private var serviceRunning = false

    /** Ticks while the app is in the foreground (or [setServiceRunning]); stops when neither holds. */
    fun setForeground(foreground: Boolean) {
        appVisible = foreground
        updateTicker()
    }

    /** The focus service keeps the ticker going while the app is in the background. */
    fun setServiceRunning(running: Boolean) {
        serviceRunning = running
        updateTicker()
    }

    private fun updateTicker() {
        val wanted = appVisible || serviceRunning
        if (wanted == (ticker != null)) return
        ticker?.cancel()
        ticker = if (wanted) scope.launch { tickLoop() } else null
    }

    private suspend fun tickLoop() {
        val core = opened.await() ?: return
        while (scope.isActive) {
            val sleepMs = tickOnce(core)
            // Wakes at the core's time, or earlier when data changed.
            withTimeoutOrNull(sleepMs) { reschedule.receive() }
        }
    }

    /** Ticks and returns how long to sleep until the next tick. */
    private fun tickOnce(core: QuestCoreApi): Long {
        val now = clock.now()
        val nextWakeMs = try {
            core.tick(now)
        } catch (e: Exception) {
            _notices.tryEmit(e.userMessage("Quest could not update. It retries in a minute."))
            return TICK_RETRY_MS
        }
        // A tick can roll the day over without changing data, so re-read anyway.
        markStale()
        return (nextWakeMs - now.epochMs).coerceAtLeast(0)
    }

    /**
     * Runs [action] on the core. Core actions only touch memory, so this is safe on the main
     * thread; errors come back as [Outcome.Failed] with the core's message. [state] follows shortly.
     */
    fun <T> perform(action: (QuestCoreApi, Moment) -> T): Outcome<T> {
        val core = core ?: return Outcome.NotReady
        val result = try {
            action(core, clock.now())
        } catch (e: Exception) {
            return Outcome.Failed(e.userMessage("That did not work. Try again."))
        }
        coreChanged()
        return Outcome.Done(result)
    }

    /** Waits until the core is open; false if it failed to open. */
    suspend fun awaitOpen(): Boolean = opened.await() != null

    /**
     * Opens the core if needed (a cold process: worker, widget) and waits on [io] until it synced;
     * null when it could not open or the call failed. The core's wait cannot be interrupted, so this
     * waits in slices of [SYNC_SLICE_MS] and checks for cancellation between them: a stopped worker
     * ends after at most one slice.
     *
     * An offline core (not signed out or failed) is asked again each slice until the time is up:
     * Android blocks a cached background process's network, so a core kept alive between jobs can
     * still be offline from retries that failed meanwhile when the next job starts. Its next retry
     * comes within 30 s, now with the job's network. The wait between those calls is not counted
     * against [timeoutMs] separately: an offline core answers at once, so the slices measure the
     * wait. A core that turns offline only at the end of a slice can stretch the total to about
     * twice [timeoutMs].
     */
    suspend fun syncUntilIdle(timeoutMs: Long): SyncOutcome? {
        var remainingMs = timeoutMs
        while (true) {
            val sliceMs = minOf(SYNC_SLICE_MS, remainingMs)
            val (outcome, retrying) = blocking { core ->
                val outcome = core.syncUntilIdle(sliceMs)
                outcome to (outcome == SyncOutcome.OFFLINE && core.syncState() is SyncState.Offline)
            } ?: return null
            remainingMs -= sliceMs
            if (remainingMs <= 0) return outcome
            when {
                // The core waited the whole slice; a cancelled caller already stopped in [blocking].
                outcome == SyncOutcome.TIMED_OUT -> continue
                retrying -> delay(sliceMs)
                else -> return outcome
            }
        }
    }

    /** Waits on [io] until local changes are on disk; false on timeout, failure or a core that did not open. */
    suspend fun flush(timeoutMs: Long): Boolean = blocking { it.flush(timeoutMs) } ?: false

    private suspend fun <T> blocking(call: (QuestCoreApi) -> T): T? {
        val core = opened.await() ?: return null
        return withContext(io) {
            try {
                call(core)
            } catch (e: Exception) {
                _notices.tryEmit(e.userMessage("Quest could not save or sync in the background."))
                null
            }
        }
    }

    /** Home asks for [query] from now on (filter or quest pick changed); [state] follows shortly. */
    fun setHomeQuery(query: HomeQuery) {
        if (query == homeQuery) return
        homeQuery = query
        markStale()
    }

    /** The plan asks for [query] from now on (null: the plan is not shown); [state] follows shortly. */
    fun setPlanQuery(query: PlanQuery?) {
        if (query == planQuery) return
        planQuery = query
        markStale()
    }

    /** The calendar asks for [query] from now on (null: it is not shown); [state] follows shortly. */
    fun setCalendarQuery(query: CalendarQuery?) {
        if (query == calendarQuery) return
        calendarQuery = query
        markStale()
    }

    /** The Quests tab is shown or not; [state] follows shortly. */
    fun setQuestsShown(shown: Boolean) {
        if (shown == questsShown) return
        questsShown = shown
        markStale()
    }

    /** The quest page shows [questId] (null: closed); [state] follows shortly. */
    fun setQuestPage(questId: String?) {
        if (questId == questPageId) return
        questPageId = questId
        markStale()
    }

    /** The review asks for [query] (null: not shown); [state] follows shortly. */
    fun setReviewQuery(query: ReviewQuery?) {
        if (query == reviewQuery) return
        reviewQuery = query
        markStale()
    }

    /** The archive is shown or not; [state] follows shortly. */
    fun setArchiveShown(shown: Boolean) {
        if (shown == archiveShown) return
        archiveShown = shown
        markStale()
    }

    /**
     * Reads from the core without marking it changed (for screens that derive data from [state],
     * such as the task sheet). Errors come back as [Outcome.Failed].
     */
    fun <T> read(query: (QuestCoreApi, Moment) -> T): Outcome<T> {
        val core = core ?: return Outcome.NotReady
        return try {
            Outcome.Done(query(core, clock.now()))
        } catch (e: Exception) {
            Outcome.Failed(e.userMessage("Could not read this task."))
        }
    }

    /** [read] on [io], for calls that block (the AI capture's network call); waits until the core is open. */
    suspend fun <T> readOnIo(query: (QuestCoreApi, Moment) -> T): Outcome<T> {
        if (!awaitOpen()) return Outcome.NotReady
        return withContext(io) { read(query) }
    }

    /** The network is back (worker start): the core retries sync now instead of after its backoff. */
    suspend fun reconnectNow() {
        blocking { it.reconnectNow() }
    }

    /** Re-reads everything the screens show. Only the reader coroutine calls this. */
    private fun publish(core: QuestCoreApi) {
        val now = clock.now()
        _state.value = try {
            val home = core.home(now, homeQuery)
            val query = planQuery
            // Leaving on the reader, so no plan read can come after it and remember the window again.
            if (query == null && planRead) core.leavePlan()
            planRead = query != null
            val plan = query?.let { core.plan(it.window, it.questFilter, now) }
            val calendar = calendarQuery?.let { core.calendar(it, now) }
            val questPage = questPageId?.let { id ->
                try {
                    QuestPage.Shown(core.questDetail(id, now))
                } catch (_: MobileException.UnknownQuest) {
                    QuestPage.Gone(id)
                }
            }
            CoreState.Ready(
                home = home,
                todayGrid = core.dayGrid(home.dayIso, now),
                sync = core.syncState(),
                account = core.account(),
                settings = core.settings(),
                workspaces = core.workspaces(now),
                plan = plan,
                calendar = calendar,
                quests = if (questsShown) core.quests(now) else null,
                questPage = questPage,
                review = reviewQuery?.let { core.review(it.week, now) },
                archive = if (archiveShown) core.archive(now) else null,
            )
        } catch (e: Exception) {
            _notices.tryEmit(e.userMessage("Could not read today's tasks."))
            return
        }
    }
}
