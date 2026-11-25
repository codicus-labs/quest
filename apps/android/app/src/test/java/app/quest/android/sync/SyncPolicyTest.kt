package app.quest.android.sync

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.quest_mobile.SyncOutcome
import uniffi.quest_mobile.SyncState

class SyncPolicyTest {
    private val live = SyncState.Live(0u)

    @Test
    fun idleIsDoneOfflineAndTimedOutTryAgain() {
        assertEquals(SyncRunResult.Success, syncRunResult(SyncOutcome.IDLE, live))
        assertEquals(SyncRunResult.Retry, syncRunResult(SyncOutcome.OFFLINE, SyncState.Offline(2u, 30u)))
        assertEquals(SyncRunResult.Retry, syncRunResult(SyncOutcome.TIMED_OUT, SyncState.Connecting))
    }

    @Test
    fun signedOutHasNothingToSync() {
        assertEquals(SyncRunResult.Success, syncRunResult(SyncOutcome.OFFLINE, SyncState.SignedOut))
    }

    @Test
    fun aCoreThatDidNotOpenFails() {
        assertEquals(SyncRunResult.Failure, syncRunResult(null, null))
    }

    @Test
    fun pendingChangesNeedABackgroundSync() {
        assertTrue(hasPendingChanges(SyncState.Live(1u)))
        assertTrue(hasPendingChanges(SyncState.Offline(3u, 10u)))
        assertTrue(hasPendingChanges(SyncState.Connecting))
        assertFalse(hasPendingChanges(live))
        assertFalse(hasPendingChanges(SyncState.Offline(0u, 10u)))
        assertFalse(hasPendingChanges(SyncState.SignedOut))
        assertFalse(hasPendingChanges(SyncState.Failed("Session expired")))
    }
}
