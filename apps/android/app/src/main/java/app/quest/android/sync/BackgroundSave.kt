package app.quest.android.sync

import android.content.Context
import app.quest.android.core.CoreHolder
import app.quest.android.core.Outcome

/** How long leaving the app (or a widget check) waits for local changes to reach disk. */
const val FLUSH_TIMEOUT_MS = 2_000L

/**
 * Saves before the process may be killed: waits until local changes are on disk, then, if changes
 * still wait for the server, queues a one-shot [SyncWorker] to carry them out in the background.
 */
suspend fun saveForBackground(context: Context, core: CoreHolder) {
    core.flush(FLUSH_TIMEOUT_MS)
    val sync = (core.read { c, _ -> c.syncState() } as? Outcome.Done)?.value ?: return
    if (hasPendingChanges(sync)) SyncScheduler.syncSoon(context)
}
