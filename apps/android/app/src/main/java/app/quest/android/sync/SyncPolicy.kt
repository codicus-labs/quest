package app.quest.android.sync

import uniffi.quest_mobile.SyncOutcome
import uniffi.quest_mobile.SyncState

/** What a background sync run tells WorkManager. */
enum class SyncRunResult { Success, Retry, Failure }

/**
 * A finished [SyncOutcome] as WorkManager's result: Idle is done; Offline and TimedOut try again
 * with the request's exponential backoff, unless nobody is signed in (nothing to sync, so done).
 * null: the core did not open (`Failure`; a periodic run comes again next period).
 */
fun syncRunResult(outcome: SyncOutcome?, sync: SyncState?): SyncRunResult = when {
    outcome == null -> SyncRunResult.Failure
    sync == SyncState.SignedOut -> SyncRunResult.Success
    outcome == SyncOutcome.IDLE -> SyncRunResult.Success
    else -> SyncRunResult.Retry
}

/**
 * Whether changes may still wait for the server, so a one-shot sync should carry them out in the
 * background. Connecting reports no count yet, so it counts as pending; signed out, failed or an expired
 * sign-in (needs the user) does not.
 */
fun hasPendingChanges(sync: SyncState): Boolean = when (sync) {
    is SyncState.Live -> sync.pending > 0u
    is SyncState.Offline -> sync.pending > 0u
    SyncState.Connecting -> true
    SyncState.SignedOut, SyncState.SessionExpired, is SyncState.Failed -> false
}
