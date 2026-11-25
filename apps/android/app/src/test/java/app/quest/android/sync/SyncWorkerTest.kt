package app.quest.android.sync

import androidx.work.ListenableWorker
import androidx.work.testing.TestListenableWorkerBuilder
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.core.CoreHolder
import app.quest.android.core.SYNC_SLICE_MS
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.quest_mobile.SyncOutcome
import uniffi.quest_mobile.SyncState

/** The worker over a [FakeCore]; the core opens only when the worker asks (a cold process). */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], application = TestSyncApp::class)
class SyncWorkerTest {
    private val app = RuntimeEnvironment.getApplication() as TestSyncApp
    private val core = FakeCore(sync = SyncState.Live(0u))
    private var opens = 0

    /** A fresh holder: nothing has opened the core yet, as when WorkManager starts the process. */
    private fun TestScope.coldProcess(open: () -> FakeCore = { core }) {
        app.core = CoreHolder(backgroundScope, StandardTestDispatcher(testScheduler), FakeClock()) {
            opens++
            open()
        }
    }

    private suspend fun runWorker(): ListenableWorker.Result = TestListenableWorkerBuilder<SyncWorker>(app).build().doWork()

    @Test
    fun aColdStartOpensTheCoreSyncsAndRedrawsTheWidgets() = runTest {
        coldProcess()

        assertEquals(ListenableWorker.Result.success(), runWorker())
        assertEquals(1, opens)
        // The job runs with network: the core retries now, not after its backoff.
        assertEquals(1, core.reconnects)
        assertEquals(listOf(SYNC_SLICE_MS), core.syncWaits)
        assertEquals(1, app.widgetRefreshes)
    }

    @Test
    fun offlineRetriesWithBackoff() = runTest {
        core.syncOutcome = SyncOutcome.OFFLINE
        core.sync = SyncState.Offline(2u, 30u)
        coldProcess()

        assertEquals(ListenableWorker.Result.retry(), runWorker())
        assertEquals(1, app.widgetRefreshes)
    }

    @Test
    fun aTimeoutRetries() = runTest {
        core.syncOutcome = SyncOutcome.TIMED_OUT
        coldProcess()

        assertEquals(ListenableWorker.Result.retry(), runWorker())
    }

    @Test
    fun aStoppedWorkerEndsAfterTheCurrentSliceWithoutCallingTheCoreAgain() = runTest {
        core.syncOutcome = SyncOutcome.TIMED_OUT
        coldProcess()
        lateinit var work: Job
        core.onSync = { work.cancel() }
        work = launch { runWorker() }

        work.join()

        assertEquals(true, work.isCancelled)
        assertEquals(listOf(SYNC_SLICE_MS), core.syncWaits)
        assertEquals(0, app.widgetRefreshes)
    }

    @Test
    fun signedOutIsDoneWithoutRetrying() = runTest {
        core.syncOutcome = SyncOutcome.OFFLINE
        core.sync = SyncState.SignedOut
        coldProcess()

        assertEquals(ListenableWorker.Result.success(), runWorker())
    }

    @Test
    fun aCoreThatCannotOpenFailsAndLeavesTheWidgets() = runTest {
        coldProcess { throw IllegalStateException("disk full") }

        assertEquals(ListenableWorker.Result.failure(), runWorker())
        assertEquals(0, app.widgetRefreshes)
    }
}
