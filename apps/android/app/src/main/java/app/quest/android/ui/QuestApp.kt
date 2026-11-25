package app.quest.android.ui

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.quest.android.R
import app.quest.android.quickadd.QuickAddActivity
import app.quest.android.ui.lists.RowActions
import app.quest.android.settings.FocusPreferences
import app.quest.android.settings.FocusPrefs
import app.quest.android.ui.account.AccountActions
import app.quest.android.ui.account.AccountDialogActions
import app.quest.android.ui.account.AccountDialogs
import app.quest.android.ui.account.AccountScreen
import app.quest.android.ui.account.AccountViewModel
import app.quest.android.ui.archive.ArchiveViewModel
import app.quest.android.ui.components.QuestSnackbarHost
import app.quest.android.ui.focus.FocusActions
import app.quest.android.ui.focus.FocusScreen
import app.quest.android.ui.focus.FocusUiState
import app.quest.android.ui.focus.FocusViewModel
import app.quest.android.ui.lists.ListViewModel
import app.quest.android.ui.logwork.LogWorkActions
import app.quest.android.ui.logwork.LogWorkSheet
import app.quest.android.ui.logwork.LogWorkViewModel
import app.quest.android.ui.logwork.TaskPickerActions
import app.quest.android.ui.navigation.DetailScaffold
import app.quest.android.ui.navigation.QuestScaffold
import app.quest.android.ui.navigation.Tab
import app.quest.android.ui.plan.PlanActions
import app.quest.android.ui.plan.PlanScreen
import app.quest.android.ui.plan.PlanViewModel
import app.quest.android.ui.quests.QuestEditorViewModel
import app.quest.android.ui.quests.QuestNavViewModel
import app.quest.android.ui.review.ReviewViewModel
import app.quest.android.ui.search.SearchViewModel
import app.quest.android.ui.settings.AiConnectionsActions
import app.quest.android.ui.settings.AiFeaturesActions
import app.quest.android.ui.settings.SettingsActions
import app.quest.android.ui.settings.SettingsScreen
import app.quest.android.ui.settings.SettingsViewModel
import app.quest.android.ui.shell.AccountDrawer
import app.quest.android.ui.shell.DrawerActions
import app.quest.android.ui.shell.DrawerData
import app.quest.android.ui.shell.MiniPlayer
import app.quest.android.ui.shell.PlayerActions
import app.quest.android.ui.shell.PlayerViewModel
import app.quest.android.ui.shell.TodaySegment
import app.quest.android.ui.task.TaskSheet
import app.quest.android.ui.task.TaskSheetActions
import app.quest.android.ui.task.TaskSheetViewModel
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.tracked.EntryActions
import app.quest.android.ui.tracked.EntrySheet
import app.quest.android.ui.tracked.EntryViewModel
import app.quest.android.ui.tracked.TrackedViewModel
import app.quest.android.ui.workspaces.ManageActions
import app.quest.android.ui.workspaces.ManageWorkspacesSheet
import app.quest.android.ui.workspaces.TimerElsewhereBar
import app.quest.android.ui.workspaces.TimerElsewhereNote
import app.quest.android.ui.workspaces.WorkspacesUiState
import app.quest.android.ui.workspaces.WorkspacesViewModel
import java.io.Serializable
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import uniffi.quest_mobile.AppView
import uniffi.quest_mobile.ListKind

/** Pages opened on top of the tabs. */
enum class Detail(@StringRes val title: Int) {
    Search(R.string.search),
    Account(R.string.account),
    Settings(R.string.settings),
    Archive(R.string.archive),
    Review(R.string.nav_review),
}

/** Where the Quests tab is: the quests, one quest's list, or the Logbook. */
sealed interface QuestsPage : Serializable {
    data object Nav : QuestsPage

    data class Quest(val questId: String) : QuestsPage

    data object Logbook : QuestsPage
}

class AppViewModels(
    val list: ListViewModel,
    val taskSheet: TaskSheetViewModel,
    val account: AccountViewModel,
    val settings: SettingsViewModel,
    val focus: FocusViewModel,
    val plan: PlanViewModel,
    val logWork: LogWorkViewModel,
    val questNav: QuestNavViewModel,
    val questEditor: QuestEditorViewModel,
    val tracked: TrackedViewModel,
    val entry: EntryViewModel,
    val player: PlayerViewModel,
    val review: ReviewViewModel,
    val archive: ArchiveViewModel,
    val search: SearchViewModel,
    val workspaces: WorkspacesViewModel,
)

/** The whole app: navigation plus the screens, wired to their ViewModels. */
@OptIn(ExperimentalComposeUiApi::class)
@Composable
fun QuestApp(
    viewModels: AppViewModels,
    notices: Flow<String>,
    focusPreferences: FocusPreferences,
    onNotificationsAllowed: () -> Unit = {},
    requests: Flow<AppRequest> = emptyFlow(),
    /** Whether AI capture's entries show (the search's action). */
    captureShown: Flow<Boolean> = flowOf(false),
) {
    var tab by rememberSaveable { mutableStateOf(Tab.Today) }
    var showPlan by rememberSaveable { mutableStateOf(false) }
    var questsPage by rememberSaveable { mutableStateOf<QuestsPage>(QuestsPage.Nav) }
    var drawerOpen by rememberSaveable { mutableStateOf(false) }
    var details by rememberSaveable { mutableStateOf(listOf<Detail>()) }
    val snackbar = remember { SnackbarHostState() }
    val captureEntries by captureShown.collectAsStateWithLifecycle(false)
    LaunchedEffect(notices) { notices.collect { snackbar.showSnackbar(it) } }
    val focusPrefs = focusPreferences.prefs.collectAsStateWithLifecycle(null).value
    val timerStarted = rememberNotificationPermissionRequest(focusPrefs, focusPreferences, onNotificationsAllowed)
    val focus = viewModels.focus
    val focusState by focus.state.collectAsStateWithLifecycle()
    ShowMessages(focus.message, focus::dismissMessage, snackbar)
    val logWork = viewModels.logWork
    val logWorkState by logWork.state.collectAsStateWithLifecycle()
    ShowMessages(logWork.message, logWork::dismissMessage, snackbar)
    val entry = viewModels.entry
    val entryState by entry.state.collectAsStateWithLifecycle()
    ShowMessages(entry.message, entry::dismissMessage, snackbar)
    val player = viewModels.player
    val playerCard by player.state.collectAsStateWithLifecycle()
    ShowMessages(player.message, player::dismissMessage, snackbar)
    val sheet = viewModels.taskSheet
    val sheetState by sheet.state.collectAsStateWithLifecycle()
    ShowMessages(sheet.message, sheet::dismissMessage, snackbar)

    val open = { detail: Detail -> details = details + detail }
    val openQuest = { questId: String ->
        details = emptyList()
        tab = Tab.Quests
        questsPage = QuestsPage.Quest(questId)
    }
    val openSearch = {
        viewModels.search.reset()
        open(Detail.Search)
    }
    BackHandler(enabled = details.isEmpty() && tab == Tab.Quests && questsPage != QuestsPage.Nav) { questsPage = QuestsPage.Nav }
    BackHandler(enabled = details.isNotEmpty()) { details = details.dropLast(1) }
    LaunchedEffect(requests) {
        requests.collect { request ->
            // Whatever covers the tabs steps aside, so the asked sheet is on screen.
            details = emptyList()
            drawerOpen = false
            if (focus.state.value != FocusUiState.Hidden) focus.close()
            when (request) {
                is AppRequest.OpenTask -> sheet.open(request.taskId)
                AppRequest.LogWork -> logWork.open()
            }
        }
    }
    val workspaces = viewModels.workspaces
    val workspacesState by workspaces.state.collectAsStateWithLifecycle()
    val workspacesData = (workspacesState as? WorkspacesUiState.Ready)?.data
    ShowMessages(workspaces.message, workspaces::dismissMessage, snackbar)
    LaunchedEffect(workspaces) {
        workspaces.switched.collect {
            viewModels.leaveWorkspace()
            if (questsPage is QuestsPage.Quest) questsPage = QuestsPage.Nav
        }
    }
    val switchTo = { id: String ->
        val isLive = workspacesData?.live?.any { it.id == id } == true
        if (isLive) {
            viewModels.leaveWorkspace()
            if (questsPage is QuestsPage.Quest) questsPage = QuestsPage.Nav
        }
        workspaces.switchTo(id)
    }
    val showTimer = {
        val timer = workspacesData?.timerElsewhere
        if (timer != null && switchTo(timer.workspaceId)) focus.open()
    }
    val accountVm = viewModels.account
    val accountState by accountVm.state.collectAsStateWithLifecycle()
    val dark = QuestTheme.isDark
    val searchNavigation = SearchNavigation(
        close = { details = details.filter { it != Detail.Search } },
        show = { view ->
            when (view) {
                AppView.HOME, AppView.PLAN -> {
                    tab = Tab.Today
                    showPlan = view == AppView.PLAN
                }
                AppView.QUESTS -> {
                    tab = Tab.Quests
                    questsPage = QuestsPage.Nav
                }
                AppView.REVIEW -> open(Detail.Review)
                AppView.FOCUS -> focus.open()
                AppView.ARCHIVE -> open(Detail.Archive)
            }
        },
        openQuest = openQuest,
        openAccount = { open(Detail.Account) },
        toggleTheme = { viewModels.settings.setAppearance(toggledAppearance(dark)) },
        openWorkspaces = workspaces::openManage,
    )
    val drawerAction = { action: () -> Unit ->
        {
            drawerOpen = false
            action()
        }
    }

    // Keeps the app's saved UI state (scroll positions, open menus) while Focus replaces it.
    val saveableStates = rememberSaveableStateHolder()
    Box(Modifier.semantics { testTagsAsResourceId = true }) {
        if (focusState != FocusUiState.Hidden) {
            // The tabs are not composed under Focus, so neither touch nor TalkBack can reach them.
            FocusScreen(focusState, focusActions(focus, timerStarted), keepScreenOn = focusPrefs?.keepScreenOn ?: true) {
                workspacesData?.timerElsewhere?.let { TimerElsewhereNote(it, showTimer) }
            }
            QuestSnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).navigationBarsPadding())
        } else {
            saveableStates.SaveableStateProvider("app") {
                when (val detail = details.lastOrNull()) {
                    null -> QuestScaffold(
                        tab = tab,
                        onTab = { picked ->
                            // Tapping Quests again goes back to the quests.
                            if (picked == Tab.Quests && tab == Tab.Quests) questsPage = QuestsPage.Nav
                            tab = picked
                        },
                        onSearch = openSearch,
                        onAccount = { drawerOpen = true },
                        workspaceName = workspacesData?.active?.name.orEmpty(),
                        leading = {
                            when {
                                tab == Tab.Today -> TodaySegment(showPlan, onChange = { showPlan = it })
                                tab == Tab.Quests && questsPage != QuestsPage.Nav -> BackToQuests { questsPage = QuestsPage.Nav }
                            }
                        },
                        timerNote = { TimerElsewhereBar(workspacesData, showTimer) },
                        player = { MiniPlayer(playerCard, playerActions(viewModels, timerStarted)) },
                        snackbarHost = { QuestSnackbarHost(snackbar) },
                    ) {
                        TabContent(tab, showPlan, questsPage, viewModels, snackbar, timerStarted, openQuest, openPage = { questsPage = it }, openReview = { open(Detail.Review) })
                    }
                    else -> DetailPage(detail, viewModels, snackbar, DetailNavigation(open, back = { details = details.dropLast(1) }, searchNavigation, timerStarted, captureEntries, openQuest))
                }
            }
            if (drawerOpen && workspacesData != null) {
                AccountDrawer(
                    DrawerData(workspacesData, accountState.account, accountState.sync),
                    DrawerActions(
                        close = { drawerOpen = false },
                        switchTo = { id -> drawerAction { switchTo(id) }() },
                        manageWorkspaces = drawerAction(workspaces::openManage),
                        openAccount = drawerAction { open(Detail.Account) },
                        signOut = accountVm::askSignOut,
                        openSettings = drawerAction { open(Detail.Settings) },
                        openArchive = drawerAction { open(Detail.Archive) },
                    ),
                )
            }
            // On the Account page its own screen shows them.
            if (Detail.Account !in details) {
                accountState.dialog?.let { AccountDialogs(it, AccountDialogActions(accountVm::answerImport, accountVm::signOut, accountVm::cancelSignOut)) }
            }
            ManageWorkspacesSheet(workspacesState, manageActions(workspaces))
            TaskSheet(sheetState, taskSheetActions(sheet, entry::open))
            EntrySheet(entryState, entryActions(entry))
        }
        LogWorkSheet(logWorkState, logWorkActions(logWork))
    }
}

/** What the open tab shows; each destination asks the core only for what it shows. */
@Composable
private fun TabContent(
    tab: Tab,
    showPlan: Boolean,
    questsPage: QuestsPage,
    viewModels: AppViewModels,
    snackbar: SnackbarHostState,
    timerStarted: () -> Unit,
    openQuest: (String) -> Unit,
    openPage: (QuestsPage) -> Unit,
    openReview: () -> Unit,
) {
    when (tab) {
        Tab.Today -> if (showPlan) PlanDestination(viewModels, snackbar, timerStarted) else ListDestination(ListKind.Today, viewModels, snackbar, timerStarted)
        Tab.Inbox -> ListDestination(ListKind.Inbox, viewModels, snackbar, timerStarted)
        Tab.Quests -> {
            when (questsPage) {
                QuestsPage.Nav -> QuestNavDestination(viewModels, openQuest, openLogbook = { openPage(QuestsPage.Logbook) })
                QuestsPage.Logbook -> ListDestination(ListKind.Logbook, viewModels, snackbar, timerStarted)
                is QuestsPage.Quest -> ListDestination(
                    ListKind.Quest(questsPage.questId),
                    viewModels,
                    snackbar,
                    timerStarted,
                    onGone = { openPage(QuestsPage.Nav) },
                    headerActions = { EditQuestButton { viewModels.questEditor.open(questsPage.questId) } },
                )
            }
            QuestEditor(viewModels, snackbar, openQuest)
        }
        Tab.Tracked -> TrackedDestination(viewModels, openReview)
    }
}

/** The player's actions; as on a row, a started timer makes its task Focus's task. */
private fun playerActions(viewModels: AppViewModels, timerStarted: () -> Unit) = PlayerActions(
    openFocus = { viewModels.focus.open() },
    stop = viewModels.player::stop,
    start = { taskId ->
        if (viewModels.player.start(taskId)) {
            viewModels.focus.rememberTask(taskId)
            timerStarted()
        }
    },
)

private fun entryActions(vm: EntryViewModel) = EntryActions(
    close = vm::close,
    edit = vm::edit,
    save = vm::save,
    delete = vm::delete,
    bookAsOrganizing = vm::bookAsOrganizing,
    openTaskPicker = vm::openTaskPicker,
    picker = TaskPickerActions(back = vm::closeTaskPicker, editQuery = vm::editTaskQuery, pick = vm::pickTask, create = null),
)

private fun manageActions(vm: WorkspacesViewModel) = ManageActions(
    close = vm::closeManage,
    editNewName = vm::editNewName,
    create = vm::create,
    editName = vm::editName,
    rename = vm::rename,
    askDelete = vm::askDelete,
    cancelDelete = vm::cancelDelete,
    confirmDelete = vm::confirmDelete,
    restore = vm::restore,
)

/**
 * Asks for notification permission at the first timer start (Android 13+), once: the timer works
 * without it, only without the notification. Returns what to call when a timer starts.
 */
@Composable
private fun rememberNotificationPermissionRequest(prefs: FocusPrefs?, preferences: FocusPreferences, onAllowed: () -> Unit): () -> Unit {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted -> if (granted) onAllowed() }
    return remember(prefs, launcher) {
        {
            val needed = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                prefs?.notificationsAsked == false &&
                ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            if (needed) {
                scope.launch { preferences.setNotificationsAsked() }
                launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }
}

private fun focusActions(vm: FocusViewModel, timerStarted: () -> Unit) = FocusActions(
    close = vm::close,
    pause = vm::pause,
    resume = { if (vm.resume()) timerStarted() },
    complete = vm::complete,
    pick = { if (vm.pick(it)) timerStarted() },
    setQuest = vm::setQuest,
    editTitle = vm::editTitle,
    commitTitle = vm::commitTitle,
    saveDrafts = vm::commitDrafts,
    notes = vm.notes,
    checklist = vm.checklist,
    undo = vm::undo,
    dismissToast = vm::dismissToast,
)

/** The plan; the core builds its days only while it is composed (the task sheet over it keeps it). */
@Composable
private fun PlanDestination(viewModels: AppViewModels, snackbar: SnackbarHostState, timerStarted: () -> Unit) {
    val plan = viewModels.plan
    DisposableEffect(plan) {
        plan.setShown(true)
        onDispose { plan.setShown(false) }
    }
    val state by plan.state.collectAsStateWithLifecycle()
    ShowMessages(plan.message, plan::dismissMessage, snackbar)
    val context = LocalContext.current
    PlanScreen(
        state,
        PlanActions(
            rows = RowActions(
                toggle = plan::toggleTask,
                open = viewModels.taskSheet::open,
                startTimer = { taskId ->
                    if (plan.startTimer(taskId)) {
                        viewModels.focus.rememberTask(taskId)
                        timerStarted()
                    }
                },
                stopTimer = plan::stopTimer,
            ),
            add = { context.startActivity(QuickAddActivity.forDay(context, it)) },
            place = plan::placeTask,
            undo = plan::undo,
            dismissToast = plan::dismissToast,
        ),
    )
}

internal fun logWorkActions(vm: LogWorkViewModel) = LogWorkActions(
    close = vm::close,
    edit = vm::edit,
    save = vm::save,
    openTaskPicker = vm::openTaskPicker,
    closeTaskPicker = vm::closeTaskPicker,
    editTaskQuery = vm::editTaskQuery,
    pickTask = vm::pickTask,
    createTask = vm::createTask,
)

/** Shows a ViewModel's failed action as a snackbar, then clears it. */
@Composable
internal fun ShowMessages(message: StateFlow<String?>, dismiss: () -> Unit, snackbar: SnackbarHostState) {
    val text by message.collectAsStateWithLifecycle()
    LaunchedEffect(text) {
        text?.let {
            snackbar.showSnackbar(it)
            dismiss()
        }
    }
}

internal fun taskSheetActions(vm: TaskSheetViewModel, openEntry: (String) -> Unit = {}) = TaskSheetActions(
    close = vm::close,
    editTitle = vm::editTitle,
    commitTitle = vm::commitTitle,
    notes = vm.notes,
    checklist = vm.checklist,
    undo = vm::undo,
    dismissToast = vm::dismissToast,
    toggleDone = vm::toggleDone,
    setQuest = vm::setQuest,
    setDay = vm::setDay,
    delete = vm::delete,
    openRepeat = vm::openRepeat,
    editRepeat = vm::editRepeat,
    toggleWeekday = vm::toggleWeekday,
    saveRepeat = vm::saveRepeat,
    stopRepeating = vm::stopRepeating,
    closeRepeat = vm::closeRepeat,
    setDeadline = vm::setDeadline,
    questOptions = vm::questOptions,
    loggedEntries = vm::loggedEntries,
    calendar = vm::calendar,
    shiftMonth = vm::shiftMonth,
    openEntry = openEntry,
)

/** Where a detail page can go: another detail, back, a quest's list, and what search's results do. */
private class DetailNavigation(
    val open: (Detail) -> Unit,
    val back: () -> Unit,
    val search: SearchNavigation,
    val timerStarted: () -> Unit,
    val captureShown: Boolean,
    val openQuest: (String) -> Unit,
)

@Composable
private fun AccountPage(vm: AccountViewModel, navigation: DetailNavigation) {
    val state by vm.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(vm) {
        vm.browserReturned()
        onPauseOrDispose {}
    }
    AccountScreen(
        state,
        AccountActions(
            signIn = vm::signIn,
            signInAgain = vm::signInAgain,
            askSignOut = vm::askSignOut,
            serverUrlEdited = vm::serverUrlEdited,
            openSettings = { navigation.open(Detail.Settings) },
            openArchive = { navigation.open(Detail.Archive) },
            dialogs = AccountDialogActions(vm::answerImport, vm::signOut, vm::cancelSignOut),
        ),
    )
}

/** Search is its own full-screen page; the others sit in a scaffold with their title and Back. */
@Composable
private fun DetailPage(detail: Detail, viewModels: AppViewModels, snackbar: SnackbarHostState, navigation: DetailNavigation) {
    @Composable
    fun scaffolded(content: @Composable () -> Unit) =
        DetailScaffold(stringResource(detail.title), onBack = navigation.back, snackbarHost = { QuestSnackbarHost(snackbar) }, content = content)
    when (detail) {
        Detail.Search -> SearchPage(viewModels, snackbar, navigation.timerStarted, navigation.search, navigation.captureShown)
        Detail.Archive -> scaffolded { ArchiveDetail(viewModels, snackbar, navigation.openQuest) }
        Detail.Review -> scaffolded { ReviewDestination(viewModels) }
        Detail.Account -> scaffolded { AccountPage(viewModels.account, navigation) }
        Detail.Settings -> scaffolded {
            val vm = viewModels.settings
            val state by vm.state.collectAsStateWithLifecycle()
            SettingsScreen(
                state,
                SettingsActions(
                    vm::changeWeeklyTarget, vm::setAppearance, vm::setKeepScreenOn,
                    AiFeaturesActions(vm::setAiFeatures, vm::acceptAiPrivacy, vm::declineAiPrivacy),
                    AiConnectionsActions(vm::loadConnections, vm::askRevoke, vm::cancelRevoke, vm::revoke),
                ),
            )
        }
    }
}
