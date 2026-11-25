package app.quest.android.sync

import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import java.util.concurrent.TimeUnit

/** Unique work names: at most one periodic and one one-shot sync are queued at any time. */
const val PERIODIC_SYNC_WORK = "quest-sync-periodic"
const val SYNC_NOW_WORK = "quest-sync-now"

private val needsNetwork = Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()

/** Schedules [SyncWorker] runs. */
object SyncScheduler {
    /**
     * Every 15 minutes with a network. KEEP: every app start calls this, and replacing would push
     * the next run out again each time.
     */
    fun schedulePeriodic(context: Context) {
        val request = PeriodicWorkRequestBuilder<SyncWorker>(15, TimeUnit.MINUTES)
            .setConstraints(needsNetwork)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
            .build()
        WorkManager.getInstance(context).enqueueUniquePeriodicWork(PERIODIC_SYNC_WORK, ExistingPeriodicWorkPolicy.KEEP, request)
    }

    /**
     * Carries out pending changes once the app is in the background. KEEP: a queued or running run
     * already waits until the outbox is empty, newer changes included; REPLACE would cancel a run
     * mid-sync each time the user leaves the app.
     */
    fun syncSoon(context: Context) {
        val request = OneTimeWorkRequestBuilder<SyncWorker>()
            .setConstraints(needsNetwork)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
            .build()
        WorkManager.getInstance(context).enqueueUniqueWork(SYNC_NOW_WORK, ExistingWorkPolicy.KEEP, request)
    }
}
