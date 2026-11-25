package app.quest.android

import android.app.Application
import android.content.Context
import androidx.datastore.preferences.preferencesDataStore
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import app.quest.android.auth.AuthRedirects
import app.quest.android.auth.EncryptedTokenStore
import app.quest.android.auth.KeystoreKey
import app.quest.android.core.CoreHolder
import app.quest.android.core.RustQuestCore
import app.quest.android.core.RustTls
import app.quest.android.core.SystemClock
import app.quest.android.focus.FocusHost
import app.quest.android.focus.FocusNotifications
import app.quest.android.focus.FocusNotifier
import app.quest.android.capture.CaptureEntries
import app.quest.android.settings.AppearanceRepository
import app.quest.android.settings.CapturePreferences
import app.quest.android.settings.DataStoreCapturePreferences
import app.quest.android.settings.DataStoreAppearanceRepository
import app.quest.android.settings.DataStoreFocusPreferences
import app.quest.android.settings.DataStoreHomePreferences
import app.quest.android.settings.FocusPreferences
import app.quest.android.settings.HomePreferences
import app.quest.android.platform.AppShortcuts
import app.quest.android.sync.SyncHost
import app.quest.android.sync.SyncScheduler
import app.quest.android.sync.saveForBackground
import app.quest.android.widget.WidgetHost
import app.quest.android.widget.WidgetUpdater
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

private val Context.appPreferences by preferencesDataStore(name = "app")

/**
 * One Rust core per process: the activities, the focus service, the widget, the sync worker and the
 * tile share [core], which always shows the active profile (guest or signed-in account) and switches it in place;
 * a cold process (worker, widget) opens it here too. It ticks while the app is in
 * the foreground or the focus service runs. Leaving the app saves to disk and, with changes still
 * waiting for the server, queues a one-shot sync.
 */
class QuestApplication :
    Application(),
    FocusHost,
    SyncHost,
    WidgetHost {
    override lateinit var core: CoreHolder
        private set

    lateinit var appearance: AppearanceRepository
        private set

    lateinit var homePreferences: HomePreferences
        private set

    lateinit var focusPreferences: FocusPreferences
        private set

    lateinit var capturePreferences: CapturePreferences
        private set

    /** Starts and stops the focus service from the core's running timer. */
    override lateinit var focusNotifier: FocusNotifier
        private set

    override lateinit var widgets: WidgetUpdater
        private set

    override suspend fun refreshWidgets() = widgets.refresh()

    /** Browser redirects (sign-in answers, the return from signing out) for the Account screen. */
    val authRedirects = AuthRedirects()

    /**
     * The core switched to another profile (signed in to an account, or out to the guest). The widget, the focus
     * service, the sync worker, the tile and share already follow the core's change signal; this redraws and
     * republishes what is only built on demand.
     */
    fun profileSwitched() {
        appScope.launch {
            widgets.refresh()
            AppShortcuts.publishStartTimer(this@QuestApplication, core)
        }
    }

    private lateinit var appScope: CoroutineScope

    override fun onCreate() {
        super.onCreate()
        // HTTPS certificate checks in Rust use Android's trust store; set that up before any connection.
        RustTls.init(this)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
        appScope = scope
        val dataDir = File(filesDir, "quest").path
        // Outside the backed-up files: the Keystore key that encrypts them never leaves this device anyway.
        val tokens = EncryptedTokenStore(File(noBackupFilesDir, "tokens"), KeystoreKey::get)
        core = CoreHolder(scope, Dispatchers.IO, SystemClock) { onChange -> RustQuestCore.open(dataDir, tokens, onChange) }
        appearance = DataStoreAppearanceRepository(appPreferences)
        homePreferences = DataStoreHomePreferences(appPreferences)
        focusPreferences = DataStoreFocusPreferences(appPreferences)
        capturePreferences = DataStoreCapturePreferences(appPreferences)
        // Package manager and shortcut service calls are binder IPC: off the main thread.
        scope.launch(Dispatchers.IO) { core.aiFeatures.collect { CaptureEntries.show(this@QuestApplication, it) } }
        FocusNotifications.createChannel(this)
        focusNotifier = FocusNotifier(this, core).also { it.start(scope) }
        widgets = WidgetUpdater(this, core).also { it.start(scope) }
        SyncScheduler.schedulePeriodic(this)
        scope.launch {
            if (core.awaitOpen()) AppShortcuts.publishStartTimer(this@QuestApplication, core)
        }
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) = core.setForeground(true)

            override fun onStop(owner: LifecycleOwner) {
                core.setForeground(false)
                // The process may be killed any time from here: get changes to disk and on their way.
                scope.launch {
                    saveForBackground(this@QuestApplication, core)
                    AppShortcuts.publishStartTimer(this@QuestApplication, core)
                }
            }
        })
    }
}
