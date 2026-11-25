package app.quest.android.core

import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.NOW_MS
import app.quest.android.PlanSamples
import app.quest.android.Samples
import app.quest.android.WorkspaceSamples
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.quest_mobile.HomeQuery
import uniffi.quest_mobile.InternalException
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.SyncOutcome
import uniffi.quest_mobile.SyncState

@OptIn(ExperimentalCoroutinesApi::class)
class CoreHolderTest {
    private val fake = FakeCore()
    private val clock = FakeClock()

    private fun TestScope.holder(open: (() -> Unit) -> QuestCoreApi = { fake }): CoreHolder {
        val dispatcher = StandardTestDispatcher(testScheduler)
        // The test clock follows the scheduler, so ticks see the time they were scheduled for.
        return CoreHolder(backgroundScope, dispatcher, { clock.epochMs = NOW_MS + testScheduler.currentTime; clock.now() }, open)
    }

    @Test
    fun opensOffTheCallerAndTicksOnceOnOpen() = runTest {
        val holder = holder()
        assertEquals(CoreState.Opening, holder.state.value)

        runCurrent()

        assertEquals(listOf(NOW_MS), fake.ticks)
        assertEquals(CoreState.Ready(Samples.home, Samples.grid, fake.sync, null, Samples.settings, WorkspaceSamples.one), holder.state.value)
    }

    @Test
    fun aiFeaturesSayNothingUntilTheCoreIsOpen() = runTest {
        fake.account = Samples.account.copy(aiEnabled = true, aiFeatures = true)
        val holder = holder()
        val seen = mutableListOf<Boolean>()
        backgroundScope.launch { holder.aiFeatures.collect { seen += it } }

        runCurrent()

        assertEquals(listOf(true), seen)
    }

    @Test
    fun failedOpenIsReportedAndActionsAreNotReady() = runTest {
        val holder = holder { throw MobileException.Storage("disk full. Free some space.") }
        runCurrent()

        assertEquals(CoreState.Failed("disk full. Free some space."), holder.state.value)
        assertEquals(Outcome.NotReady, holder.perform { c, now -> c.toggleTask("t1", now) })
    }

    @Test
    fun ticksWhenTheCoreAsksOnlyWhileInForeground() = runTest {
        fake.nextWakeInMs = 60_000
        val holder = holder()
        runCurrent()
        holder.setForeground(true)
        runCurrent()
        assertEquals(2, fake.ticks.size)

        advanceTimeBy(59_999)
        assertEquals(2, fake.ticks.size)
        advanceTimeBy(2)
        assertEquals(3, fake.ticks.size)
        assertEquals(NOW_MS + 60_000, fake.ticks.last())

        holder.setForeground(false)
        advanceTimeBy(10 * 60_000)
        assertEquals(3, fake.ticks.size)
    }

    @Test
    fun theFocusServiceKeepsTickingInTheBackground() = runTest {
        fake.nextWakeInMs = 1_000
        val holder = holder()
        holder.setForeground(true)
        holder.setServiceRunning(true)
        holder.setForeground(false)
        runCurrent()
        val before = fake.ticks.size

        advanceTimeBy(2_001)
        assertEquals(before + 2, fake.ticks.size)

        holder.setServiceRunning(false)
        advanceTimeBy(10_000)
        assertEquals(before + 2, fake.ticks.size)
    }

    @Test
    fun runningTimerTicksEverySecond() = runTest {
        fake.nextWakeInMs = 1_000
        val holder = holder()
        holder.setForeground(true)
        runCurrent()
        val before = fake.ticks.size

        advanceTimeBy(3_001)

        assertEquals(before + 3, fake.ticks.size)
        holder.setForeground(false)
    }

    @Test
    fun anActionWakesTheTickerSoANewTimerTicksEverySecondAtOnce() = runTest {
        fake.nextWakeInMs = 60_000
        val holder = holder()
        holder.setForeground(true)
        runCurrent()
        val before = fake.ticks.size

        fake.nextWakeInMs = 1_000
        holder.perform { c, now -> c.startTimer("t1", now) }
        runCurrent()
        assertEquals(before + 1, fake.ticks.size)

        advanceTimeBy(1_001)
        assertEquals(before + 2, fake.ticks.size)
        holder.setForeground(false)
    }

    @Test
    fun everyTickRereadsHomeSoTheDayRollsOver() = runTest {
        val holder = holder()
        holder.setForeground(true)
        runCurrent()
        fake.home = Samples.home.copy(dayLabel = "Tuesday, November 24")

        advanceTimeBy(60_001)

        assertEquals("Tuesday, November 24", (holder.state.value as CoreState.Ready).home.dayLabel)
        holder.setForeground(false)
    }

    @Test
    fun failedTickBecomesANoticeAndRetriesInAMinute() = runTest {
        val holder = holder()
        runCurrent()
        val notices = mutableListOf<String>()
        backgroundScope.launch(start = CoroutineStart.UNDISPATCHED) { holder.notices.collect { notices += it } }
        fake.failNext = MobileException.Internal("internal error: boom. Please report this.")

        holder.setForeground(true)
        runCurrent()
        assertEquals(listOf("internal error: boom. Please report this."), notices)

        advanceTimeBy(60_001)
        assertTrue(fake.ticks.size >= 2)
        holder.setForeground(false)
    }

    @Test
    fun actionsRefreshAndReturnCoreErrors() = runTest {
        val holder = holder()
        runCurrent()
        fake.home = Samples.emptyHome

        assertEquals(Outcome.Done(Unit), holder.perform { c, now -> c.toggleTask("t1", now) })
        runCurrent()
        assertEquals(Samples.emptyHome, (holder.state.value as CoreState.Ready).home)

        fake.failNext = MobileException.InvalidId("\"x\" is not a valid id.")
        assertEquals(Outcome.Failed("\"x\" is not a valid id."), holder.perform { c, now -> c.toggleTask("x", now) })
    }

    @Test
    fun coreChangeCallbackOnlySignalsAndBurstsCollapse() = runTest {
        var onChange: () -> Unit = {}
        val holder = holder { listener -> onChange = listener; fake }
        runCurrent()
        val readsBefore = fake.homeReads
        fake.sync = SyncState.Live(0u)

        // The callback runs on the core's worker thread; calling back into the core there could deadlock.
        repeat(5) { onChange() }
        assertEquals(readsBefore, fake.homeReads)

        // A burst collapses: one read for the signal handed to the waiting reader, at most one for the rest.
        runCurrent()
        assertTrue(fake.homeReads in readsBefore + 1..readsBefore + 2)
        assertEquals(SyncState.Live(0u), (holder.state.value as CoreState.Ready).sync)
    }

    @Test
    fun homeIsReadWithTheLatestQueryAndTodaysGrid() = runTest {
        val holder = holder()
        runCurrent()

        holder.setHomeQuery(HomeQuery(questFilter = "q1", questPick = null))
        runCurrent()

        assertEquals(HomeQuery(questFilter = "q1", questPick = null), fake.queries.last())
        assertEquals("2026-11-23", fake.gridDays.last())
    }

    @Test
    fun readsDoNotMarkTheCoreChanged() = runTest {
        val holder = holder()
        runCurrent()
        val readsBefore = fake.homeReads

        assertEquals(Outcome.Done(Samples.detail), holder.read { c, now -> c.taskDetail("t1", now) })
        runCurrent()

        assertEquals(readsBefore, fake.homeReads)
    }

    @Test
    fun unexpectedCoreErrorsBecomeFailuresInsteadOfCrashes() = runTest {
        val holder = holder()
        runCurrent()

        fake.failNext = InternalException("panicked at core.rs:91")
        val outcome = holder.perform { c, now -> c.toggleTask("t1", now) }

        assertEquals(Outcome.Failed("That did not work. Try again. (InternalException: panicked at core.rs:91)"), outcome)
    }

    @Test
    fun openFailingWithAnUnexpectedErrorIsReportedNotStuckOpening() = runTest {
        val holder = holder { throw IllegalStateException("library not loaded") }
        runCurrent()

        assertEquals(
            CoreState.Failed("Could not open Quest's data. Restart the app. (IllegalStateException: library not loaded)"),
            holder.state.value,
        )
    }

    @Test
    fun readsThePlanOnlyWhileAskedAndLeavesItOnTheReader() = runTest {
        val holder = holder()
        runCurrent()
        assertEquals(null, (holder.state.value as CoreState.Ready).plan)
        assertEquals(emptyList<Any>(), fake.planReads)

        val query = PlanQuery(PlanSamples.window, "q1")
        holder.setPlanQuery(query)
        runCurrent()
        assertEquals(PlanSamples.plan, (holder.state.value as CoreState.Ready).plan)
        assertEquals(listOf(PlanSamples.window to "q1"), fake.planReads)

        holder.setPlanQuery(null)
        runCurrent()
        assertEquals(null, (holder.state.value as CoreState.Ready).plan)
        assertEquals(listOf("leave-plan"), fake.calls)

        // Leaves once, not on every later read.
        holder.perform { c, now -> c.stopTimer(now) }
        runCurrent()
        assertEquals(listOf("leave-plan", "stop"), fake.calls)
    }

    @Test
    fun changesSignalActionsAndCallbacksButNotTicks() = runTest {
        var onChange: () -> Unit = {}
        val holder = holder { listener -> onChange = listener; fake }
        var changes = 0
        backgroundScope.launch(start = CoroutineStart.UNDISPATCHED) { holder.changes.collect { changes++ } }
        fake.nextWakeInMs = 1_000
        holder.setForeground(true)
        advanceTimeBy(3_001)
        assertEquals(0, changes)

        holder.perform { c, now -> c.toggleTask("t1", now) }
        runCurrent()
        onChange()
        runCurrent()

        assertEquals(2, changes)
        holder.setForeground(false)
    }

    @Test
    fun syncAndFlushWaitOnTheCoreWithTheirTimeouts() = runTest {
        fake.syncOutcome = SyncOutcome.OFFLINE
        val holder = holder()

        assertEquals(SyncOutcome.OFFLINE, holder.syncUntilIdle(60_000))
        assertEquals(true, holder.flush(2_000))
        assertEquals(listOf(SYNC_SLICE_MS), fake.syncWaits)
        assertEquals(listOf(2_000L), fake.flushes)
    }

    @Test
    fun syncWaitsInSlicesUntilTheTimeoutIsUsedUp() = runTest {
        fake.syncOutcome = SyncOutcome.TIMED_OUT
        val holder = holder()

        assertEquals(SyncOutcome.TIMED_OUT, holder.syncUntilIdle(2_500))
        assertEquals(listOf(1_000L, 1_000L, 500L), fake.syncWaits)
    }

    @Test
    fun syncReturnsAsSoonAsASliceEndsIdle() = runTest {
        var calls = 0
        fake.onSync = { if (++calls == 2) fake.syncOutcome = SyncOutcome.IDLE }
        fake.syncOutcome = SyncOutcome.TIMED_OUT
        val holder = holder()

        assertEquals(SyncOutcome.IDLE, holder.syncUntilIdle(60_000))
        assertEquals(2, fake.syncWaits.size)
    }

    @Test
    fun anOfflineCoreIsAskedAgainUntilItsRetryGetsThrough() = runTest {
        var calls = 0
        fake.sync = SyncState.Offline(1u, 30u)
        fake.syncOutcome = SyncOutcome.OFFLINE
        fake.onSync = { if (++calls == 3) fake.syncOutcome = SyncOutcome.IDLE }
        val holder = holder()

        assertEquals(SyncOutcome.IDLE, holder.syncUntilIdle(60_000))
        assertEquals(3, fake.syncWaits.size)
    }

    @Test
    fun anOfflineCoreStillOfflineWhenTheTimeIsUpReportsOffline() = runTest {
        fake.sync = SyncState.Offline(1u, 30u)
        fake.syncOutcome = SyncOutcome.OFFLINE
        val holder = holder()

        assertEquals(SyncOutcome.OFFLINE, holder.syncUntilIdle(2_500))
        assertEquals(listOf(1_000L, 1_000L, 500L), fake.syncWaits)
    }

    @Test
    fun aSignedOutCoreReturnsAtOnce() = runTest {
        fake.sync = SyncState.SignedOut
        fake.syncOutcome = SyncOutcome.OFFLINE
        val holder = holder()

        assertEquals(SyncOutcome.OFFLINE, holder.syncUntilIdle(60_000))
        assertEquals(listOf(SYNC_SLICE_MS), fake.syncWaits)
    }

    @Test
    fun aCancelledSyncStopsAfterTheCurrentSlice() = runTest {
        fake.syncOutcome = SyncOutcome.TIMED_OUT
        val holder = holder()
        lateinit var sync: Job
        fake.onSync = { sync.cancel() }
        sync = launch { holder.syncUntilIdle(60_000) }

        sync.join()

        assertTrue(sync.isCancelled)
        assertEquals(listOf(SYNC_SLICE_MS), fake.syncWaits)
    }

    @Test
    fun aCancelledSyncStopsWhileWaitingForAnOfflineCoresRetry() = runTest {
        fake.sync = SyncState.Offline(1u, 30u)
        fake.syncOutcome = SyncOutcome.OFFLINE
        val holder = holder()
        val sync = launch { holder.syncUntilIdle(60_000) }
        runCurrent()

        sync.cancel()
        sync.join()

        assertTrue(sync.isCancelled)
        assertEquals(listOf(SYNC_SLICE_MS), fake.syncWaits)
    }

    @Test
    fun syncAndFlushOnACoreThatDidNotOpenReportNothingDone() = runTest {
        val holder = holder { throw MobileException.Storage("disk full. Free some space.") }

        assertEquals(null, holder.syncUntilIdle(60_000))
        assertEquals(false, holder.flush(2_000))
    }

    @Test
    fun aFailingFlushBecomesANotice() = runTest {
        val holder = holder()
        runCurrent()
        val notices = mutableListOf<String>()
        backgroundScope.launch(start = CoroutineStart.UNDISPATCHED) { holder.notices.collect { notices += it } }
        fake.failNext = MobileException.Internal("internal error: boom. Please report this.")

        assertEquals(false, holder.flush(2_000))
        assertEquals(listOf("internal error: boom. Please report this."), notices)
    }
}
