package app.quest.android

import android.app.UiModeManager
import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.view.ViewTreeObserver
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.LaunchedEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import app.quest.android.auth.openInBrowser
import app.quest.android.platform.ShortcutActions
import app.quest.android.ui.AppRequest
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.quest.android.core.RustRepeatRules
import app.quest.android.core.RustServerUrlPolicy
import app.quest.android.core.SystemClock
import app.quest.android.focus.FocusNotifications
import app.quest.android.settings.Appearance
import app.quest.android.ui.AppViewModels
import app.quest.android.ui.archive.ArchiveViewModel
import app.quest.android.ui.quests.QuestNavViewModel
import app.quest.android.ui.quests.QuestEditorViewModel
import app.quest.android.ui.shell.PlayerViewModel
import app.quest.android.ui.tracked.EntryViewModel
import app.quest.android.ui.tracked.TrackedViewModel
import app.quest.android.ui.review.ReviewViewModel
import app.quest.android.ui.search.SearchViewModel
import app.quest.android.ui.QuestApp
import app.quest.android.ui.account.AccountViewModel
import app.quest.android.ui.focus.FocusViewModel
import app.quest.android.ui.lists.ListViewModel
import app.quest.android.ui.logwork.LogWorkViewModel
import app.quest.android.ui.plan.PlanViewModel
import app.quest.android.ui.settings.SettingsViewModel
import app.quest.android.ui.task.TaskSheetViewModel
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.workspaces.WorkspacesViewModel

/** quest://task/<id> opens that task's sheet (the widget's task rows). */
private const val TASK_LINK_HOST = "task"

/**
 * The not exported alias of [MainActivity] (manifest) for the widget and the launcher shortcuts. Only intents
 * through it may change data (start a timer); from other apps, [MainActivity] only opens screens.
 */
const val INTERNAL_ENTRY = "app.quest.android.InternalEntry"

class MainActivity : ComponentActivity() {
    private val app get() = application as QuestApplication

    private val list: ListViewModel by viewModels {
        viewModelFactory { initializer { ListViewModel(app.core, createSavedStateHandle()) } }
    }
    private val taskSheet: TaskSheetViewModel by viewModels {
        viewModelFactory { initializer { TaskSheetViewModel(app.core, createSavedStateHandle(), RustRepeatRules, SystemClock) } }
    }
    private val account: AccountViewModel by viewModels {
        viewModelFactory {
            initializer { AccountViewModel(app.core, RustServerUrlPolicy, createSavedStateHandle(), app.authRedirects.redirects, app::profileSwitched) }
        }
    }
    private val settings: SettingsViewModel by viewModels {
        viewModelFactory { initializer { SettingsViewModel(app.core, app.appearance, app.focusPreferences, app.capturePreferences) } }
    }
    private val focus: FocusViewModel by viewModels {
        viewModelFactory { initializer { FocusViewModel(app.core, createSavedStateHandle()) } }
    }

    private val plan: PlanViewModel by viewModels {
        viewModelFactory { initializer { PlanViewModel(app.core) } }
    }

    private val logWork: LogWorkViewModel by viewModels {
        viewModelFactory { initializer { LogWorkViewModel(app.core, createSavedStateHandle()) } }
    }
    private val questNav: QuestNavViewModel by viewModels {
        viewModelFactory { initializer { QuestNavViewModel(app.core) } }
    }
    private val questEditor: QuestEditorViewModel by viewModels {
        viewModelFactory { initializer { QuestEditorViewModel(app.core, createSavedStateHandle()) } }
    }
    private val tracked: TrackedViewModel by viewModels {
        viewModelFactory { initializer { TrackedViewModel(app.core) } }
    }
    private val entry: EntryViewModel by viewModels {
        viewModelFactory { initializer { EntryViewModel(app.core, createSavedStateHandle()) } }
    }
    private val player: PlayerViewModel by viewModels {
        viewModelFactory { initializer { PlayerViewModel(app.core) } }
    }
    private val review: ReviewViewModel by viewModels {
        viewModelFactory { initializer { ReviewViewModel(app.core, createSavedStateHandle()) } }
    }
    private val archive: ArchiveViewModel by viewModels {
        viewModelFactory { initializer { ArchiveViewModel(app.core) } }
    }
    private val search: SearchViewModel by viewModels {
        viewModelFactory { initializer { SearchViewModel(app.core, createSavedStateHandle()) } }
    }
    private val workspaces: WorkspacesViewModel by viewModels {
        viewModelFactory { initializer { WorkspacesViewModel(app.core, createSavedStateHandle()) } }
    }

    /** Deep links and shortcuts for the composed app; buffered until it collects. */
    private val requests = Channel<AppRequest>(Channel.BUFFERED)

    /** One flow for the activity: a new one per recomposition would restart the collector mid-request. */
    private val requestFlow = requests.receiveAsFlow()

    /** Sign-in and sign-out pages, only while started: one asked for while stopped is dropped, not opened later. */
    private fun openBrowserPages() {
        val noBrowser = getString(R.string.account_no_browser)
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                account.browser.collect { url -> if (!openInBrowser(this@MainActivity, url)) account.browserMissing(noBrowser) }
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // A restored activity already shows what it showed; only a fresh launch follows the link.
        if (savedInstanceState == null) handleIntent(intent)
        openBrowserPages()
        val viewModels = AppViewModels(list, taskSheet, account, settings, focus, plan, logWork, questNav, questEditor, tracked, entry, player, review, archive, search, workspaces)
        var appearanceLoaded = false
        keepSplashUntil { appearanceLoaded }
        setContent {
            // null until DataStore has read the stored choice, so the first frame already has the right theme.
            val appearance = app.appearance.appearance.collectAsStateWithLifecycle(null).value ?: return@setContent
            appearanceLoaded = true
            val dark = when (appearance) {
                Appearance.System -> isSystemInDarkTheme()
                Appearance.Light -> false
                Appearance.Dark -> true
            }
            LaunchedEffect(dark) { useSystemBarIcons(dark) }
            LaunchedEffect(appearance) { useForSplash(appearance) }
            QuestTheme(dark) { QuestApp(viewModels, app.core.notices, app.focusPreferences, onNotificationsAllowed = app.focusNotifier::repost, requests = requestFlow, captureShown = app.core.aiFeatures) }
        }
    }

    override fun onResume() {
        super.onResume()
        app.focusNotifier.appResumed()
    }

    /** Deep links and shortcuts while the activity already runs. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    /** quest://focus, quest://task/<id> (widget), and the launcher shortcuts. */
    private fun handleIntent(intent: Intent?) {
        intent ?: return
        // Reopened from Recents, the old intent comes back; it was handled when it arrived.
        if (intent.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY != 0) return
        val fromThisApp = intent.component?.className == INTERNAL_ENTRY
        when (intent.action) {
            ShortcutActions.START_TIMER -> if (fromThisApp) focus.startLastTask()
            ShortcutActions.LOG_WORK -> request(AppRequest.LogWork)
            Intent.ACTION_VIEW -> {
                val link = intent.data ?: return
                if (link.scheme != FocusNotifications.FOCUS_URI.scheme) return
                when (link.host) {
                    FocusNotifications.FOCUS_URI.host -> focus.open()
                    TASK_LINK_HOST -> link.lastPathSegment?.let { request(AppRequest.OpenTask(it)) }
                }
            }
        }
    }

    /** Hands [request] to the app once the core is open (a shortcut may have started the process). */
    private fun request(request: AppRequest) {
        lifecycleScope.launch { if (app.core.awaitOpen()) requests.send(request) }
    }

    /** Holds the first frame (the system splash stays up) until [ready]; avoids a theme flash on cold start. */
    private fun keepSplashUntil(ready: () -> Boolean) {
        val content = findViewById<View>(android.R.id.content)
        content.viewTreeObserver.addOnPreDrawListener(object : ViewTreeObserver.OnPreDrawListener {
            override fun onPreDraw(): Boolean {
                if (!ready()) return false
                content.viewTreeObserver.removeOnPreDrawListener(this)
                return true
            }
        })
    }

    /**
     * Tells the system the app's appearance, so the splash of the next start (drawn before any app code
     * runs, from `values-night`) matches it. The system keeps it across restarts; only a change recreates
     * the activity, once.
     */
    private fun useForSplash(appearance: Appearance) {
        val mode = when (appearance) {
            Appearance.System -> UiModeManager.MODE_NIGHT_AUTO
            Appearance.Light -> UiModeManager.MODE_NIGHT_NO
            Appearance.Dark -> UiModeManager.MODE_NIGHT_YES
        }
        getSystemService(UiModeManager::class.java)?.setApplicationNightMode(mode)
    }

    /** Status and navigation bar icons that stay readable when the appearance overrides the system's. */
    private fun useSystemBarIcons(dark: Boolean) {
        val style = if (dark) SystemBarStyle.dark(Color.TRANSPARENT) else SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT)
        enableEdgeToEdge(statusBarStyle = style, navigationBarStyle = style)
    }
}
