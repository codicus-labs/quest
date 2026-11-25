package app.quest.android.sync

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import app.quest.android.core.CoreHolder
import app.quest.android.core.Outcome

/** What the sync worker needs from the application (`QuestApplication`; a test app in tests). */
interface SyncHost {
    val core: CoreHolder

    /** Redraws the home-screen widgets from the core. */
    suspend fun refreshWidgets()
}

/** How long one background run waits for sync; well inside WorkManager's 10 minutes. */
const val SYNC_TIMEOUT_MS = 60_000L

/**
 * One background sync (periodic or the one-shot after the app went to the background): opens the
 * core if the process is cold, waits until it synced, redraws the widgets.
 *
 * While the app is in the foreground the core syncs live anyway; the worker still calls
 * `syncUntilIdle`, which then returns at once. One path, no foreground check that could race.
 */
class SyncWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val host = applicationContext as SyncHost
        // The job runs because the network is there: no need to wait out the core's retry backoff.
        host.core.reconnectNow()
        val outcome = host.core.syncUntilIdle(SYNC_TIMEOUT_MS)
        val sync = (host.core.read { c, _ -> c.syncState() } as? Outcome.Done)?.value
        if (outcome != null) host.refreshWidgets()
        return when (syncRunResult(outcome, sync)) {
            SyncRunResult.Success -> Result.success()
            SyncRunResult.Retry -> Result.retry()
            SyncRunResult.Failure -> Result.failure()
        }
    }
}
