package app.quest.android.ui

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import app.quest.android.settings.FocusPreferences
import app.quest.android.settings.FocusPrefs
import app.quest.android.ui.focus.FocusActions
import app.quest.android.ui.focus.FocusScreen
import app.quest.android.ui.focus.FocusUiState
import app.quest.android.ui.focus.FocusViewModel
import kotlinx.coroutines.launch
import androidx.annotation.StringRes
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.quest.android.R
import app.quest.android.core.RustPlanRules
import app.quest.android.ui.account.AccountActions
import app.quest.android.ui.account.AccountDialogActions
import app.quest.android.ui.account.AccountScreen
import app.quest.android.ui.account.AccountViewModel
import app.quest.android.ui.components.QuestSnackbarHost
import app.quest.android.ui.home.HomeActions
import app.quest.android.ui.home.HomeScreen
import app.quest.android.ui.home.HomeViewModel
import app.quest.android.ui.navigation.DetailScaffold
import app.quest.android.ui.navigation.QuestScaffold
import app.quest.android.ui.navigation.TopDestination
import app.quest.android.ui.calendar.CalendarActions
import app.quest.android.ui.calendar.CalendarGridActions
import app.quest.android.ui.calendar.CalendarScreen
import app.quest.android.ui.calendar.CalendarSheetActions
import app.quest.android.ui.calendar.CalendarSheets
import app.quest.android.ui.calendar.CalendarViewModel
import app.quest.android.ui.calendar.calendarDayCount
import androidx.compose.foundation.layout.BoxWithConstraints
import uniffi.quest_mobile.GridBlockKind
import app.quest.android.ui.plan.PlanActions
import app.quest.android.ui.plan.PlanScreen
import app.quest.android.ui.plan.PlanViewModel
import app.quest.android.ui.settings.AiConnectionsActions
import app.quest.android.ui.settings.AiFeaturesActions
import app.quest.android.ui.settings.SettingsActions
import app.quest.android.ui.settings.SettingsScreen
import app.quest.android.ui.settings.SettingsViewModel
import androidx.compose.foundation.layout.Box
import app.quest.android.ui.task.TaskSheet
import app.quest.android.ui.task.TaskSheetActions
import app.quest.android.ui.task.TaskSheetViewModel
import app.quest.android.ui.archive.ArchiveViewModel
import app.quest.android.ui.quests.QuestDetailUiState
import app.quest.android.ui.quests.QuestDetailViewModel
import app.quest.android.ui.quests.QuestEditorViewModel
import app.quest.android.ui.quests.QuestsViewModel
import app.quest.android.ui.review.ReviewViewModel
import app.quest.android.ui.search.SearchViewModel
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.workspaces.ManageActions
import app.quest.android.ui.workspaces.ManageWorkspacesSheet
import app.quest.android.ui.workspaces.SwitcherActions
import app.quest.android.ui.workspaces.TimerElsewhereNote
import app.quest.android.ui.workspaces.WorkspacesUiState
import app.quest.android.ui.workspaces.WorkspacesViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.StateFlow

/** Screens opened from the top bar, on top of the current destination. */
enum class Detail(@StringRes val title: Int) { Search(R.string.search), Account(R.string.account), Settings(R.string.settings), Archive(R.string.archive) }

class AppViewModels(
    val home: HomeViewModel,
    val taskSheet: TaskSheetViewModel,
    val account: AccountViewModel,
    val settings: SettingsViewModel,
    val focus: FocusViewModel,
    val plan: PlanViewModel,
    val calendar: CalendarViewModel,
    val quests: QuestsViewModel,
    val questEditor: QuestEditorViewModel,
    val questDetail: QuestDetailViewModel,
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
    var destination by rememberSaveable { mutableStateOf(TopDestination.Home) }
    var details by rememberSaveable { mutableStateOf(listOf<Detail>()) }
    val snackbar = remember { SnackbarHostState() }
    val captureEntries by captureShown.collectAsStateWithLifecycle(false)
    LaunchedEffect(notices) { notices.collect { snackbar.showSnackbar(it) } }
    val focusPrefs = focusPreferences.prefs.collectAsStateWithLifecycle(null).value
    val timerStarted = rememberNotificationPermissionRequest(focusPrefs, focusPreferences, onNotificationsAllowed)
    val focus = viewModels.focus
    val focusState by focus.state.collectAsStateWithLifecycle()
    ShowMessages(focus.message, focus::dismissMessage, snackbar)

    val open = { detail: Detail -> details = details + detail }
    val openSearch = {
        viewModels.search.reset()
        open(Detail.Search)
    }
    BackHandler(enabled = details.isNotEmpty()) { details = details.dropLast(1) }
    LaunchedEffect(requests) {
        requests.collect { request ->
            // Whatever covers the tabs steps aside, so the asked sheet is on screen.
            details = emptyList()
            if (focus.state.value != FocusUiState.Hidden) focus.close()
            when (request) {
                is AppRequest.OpenTask -> viewModels.taskSheet.open(request.taskId)
                AppRequest.LogWork -> {
                    viewModels.questDetail.close()
                    destination = TopDestination.Calendar
                    viewModels.calendar.openLogWork()
                }
            }
        }
    }
    val questPageState by viewModels.questDetail.state.collectAsStateWithLifecycle()
    val workspaces = viewModels.workspaces
    val workspacesState by workspaces.state.collectAsStateWithLifecycle()
    ShowMessages(workspaces.message, workspaces::dismissMessage, snackbar)
    LaunchedEffect(workspaces) {
        workspaces.switched.collect { if (viewModels.leaveWorkspace()) destination = TopDestination.Quests }
    }
    val switcher = switcherActions(viewModels, showQuests = { destination = TopDestination.Quests })
    val dark = QuestTheme.isDark
    val searchNavigation = SearchNavigation(
        close = { details = details.filter { it != Detail.Search } },
        goTo = { destination = it },
        openArchive = { open(Detail.Archive) },
        openAccount = { open(Detail.Account) },
        toggleTheme = { viewModels.settings.setAppearance(toggledAppearance(dark)) },
        openWorkspaces = workspaces::openManage,
    )

    // Keeps the app's saved UI state (scroll positions, open menus) while Focus replaces it.
    val saveableStates = rememberSaveableStateHolder()
    Box(Modifier.semantics { testTagsAsResourceId = true }) {
        if (focusState != FocusUiState.Hidden) {
            // Home is not composed under Focus, so neither touch nor TalkBack can reach it.
            FocusScreen(focusState, focusActions(focus, timerStarted), keepScreenOn = focusPrefs?.keepScreenOn ?: true) {
                (workspacesState as? WorkspacesUiState.Ready)?.data?.timerElsewhere?.let { TimerElsewhereNote(it, switcher.showTimer) }
            }
            QuestSnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).navigationBarsPadding())
        } else if (questPageState != QuestDetailUiState.Hidden) {
            QuestDetailPage(viewModels, snackbar, timerStarted)
        } else {
            saveableStates.SaveableStateProvider("app") {
                when (val detail = details.lastOrNull()) {
                    null -> QuestScaffold(
                        destination = destination,
                        onNavigate = { destination = it },
                        onSearch = openSearch,
                        onAccount = { open(Detail.Account) },
                        onFocus = { focus.open() },
                        workspaces = (workspacesState as? WorkspacesUiState.Ready)?.data,
                        switcher = switcher,
                        snackbarHost = { QuestSnackbarHost(snackbar) },
                    ) { DestinationContent(destination, viewModels, snackbar, timerStarted, openArchive = { open(Detail.Archive) }) }
                    else -> DetailPage(detail, viewModels, snackbar, DetailNavigation(open, back = { details = details.dropLast(1) }, searchNavigation, timerStarted, captureEntries))
                }
            }
            ManageWorkspacesSheet(workspacesState, manageActions(workspaces))
        }
    }
}

/**
 * The switcher's actions. Sheets save into their own workspace before the switch; a quest's page
 * gives way to Quests (the same happens once the core shows the other workspace, see `switched`).
 * A workspace no longer live is left to the core to refuse, without leaving the shown one first.
 */
private fun switcherActions(viewModels: AppViewModels, showQuests: () -> Unit): SwitcherActions {
    val switchTo = { id: String ->
        val data = (viewModels.workspaces.state.value as? WorkspacesUiState.Ready)?.data
        val isLive = data?.live?.any { it.id == id } == true
        if (isLive && viewModels.leaveWorkspace()) showQuests()
        viewModels.workspaces.switchTo(id)
    }
    return SwitcherActions(
        switchTo = { switchTo(it) },
        manage = viewModels.workspaces::openManage,
        showTimer = {
            val timer = (viewModels.workspaces.state.value as? WorkspacesUiState.Ready)?.data?.timerElsewhere
            if (timer != null && switchTo(timer.workspaceId)) viewModels.focus.open()
        },
    )
}

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
    editDescription = vm::editDescription,
    commitDescription = vm::commitDescription,
    saveDrafts = vm::commitDrafts,
)

@Composable
private fun DestinationContent(destination: TopDestination, viewModels: AppViewModels, snackbar: SnackbarHostState, timerStarted: () -> Unit, openArchive: () -> Unit) {
    val sheet = viewModels.taskSheet
    val sheetState by sheet.state.collectAsStateWithLifecycle()
    ShowMessages(sheet.message, sheet::dismissMessage, snackbar)
    when (destination) {
        TopDestination.Plan -> PlanDestination(viewModels, snackbar, timerStarted)
        TopDestination.Calendar -> CalendarDestination(viewModels, snackbar)
        TopDestination.Quests -> QuestsDestination(viewModels, snackbar, openArchive)
        TopDestination.Review -> ReviewDestination(viewModels)
        TopDestination.Home -> {
            val home = viewModels.home
            val state by home.state.collectAsStateWithLifecycle()
            ShowMessages(home.message, home::dismissMessage, snackbar)
            HomeScreen(state, homeActions(home, sheet, viewModels.focus, timerStarted))
        }
    }
    TaskSheet(sheetState, taskSheetActions(sheet))
}

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
    PlanScreen(state, planActions(plan, viewModels.taskSheet, viewModels.focus, timerStarted))
}

/**
 * The calendar; the core builds its page only while it is composed, with as many days as fit
 * (1 compact, 3 medium, 7 expanded).
 */
@Composable
private fun CalendarDestination(viewModels: AppViewModels, snackbar: SnackbarHostState) {
    val calendar = viewModels.calendar
    val state by calendar.state.collectAsStateWithLifecycle()
    ShowMessages(calendar.message, calendar::dismissMessage, snackbar)
    BoxWithConstraints {
        val dayCount = calendarDayCount(maxWidth)
        DisposableEffect(calendar, dayCount) {
            calendar.setShown(true, dayCount)
            onDispose { calendar.setShown(false) }
        }
        CalendarScreen(state, calendarActions(calendar, viewModels.taskSheet))
    }
    CalendarSheets(state, calendarSheetActions(calendar, viewModels.taskSheet))
}

internal fun calendarActions(vm: CalendarViewModel, sheet: TaskSheetViewModel) = CalendarActions(
    previous = vm::previous,
    next = vm::next,
    today = vm::today,
    openLogWork = vm::openLogWork,
    undo = vm::undo,
    dismissToast = vm::dismissToast,
    grid = CalendarGridActions(
        preview = vm::preview,
        drop = vm::drop,
        tapBlock = { block -> if (block.kind == GridBlockKind.PLANNED) sheet.open(block.targetId) else vm.openEntry(block.targetId) },
        edgeSpeed = RustPlanRules::edgeSpeed,
    ),
)

internal fun calendarSheetActions(vm: CalendarViewModel, sheet: TaskSheetViewModel) = CalendarSheetActions(
    close = vm::closeSheet,
    editLogWork = vm::editLogWork,
    saveLogWork = vm::saveLogWork,
    editEntry = vm::editEntry,
    saveEntry = vm::saveEntry,
    deleteEntry = vm::deleteEntry,
    openTask = { taskId ->
        vm.closeSheet()
        sheet.open(taskId)
    },
    openTaskPicker = vm::openTaskPicker,
    closeTaskPicker = vm::closeTaskPicker,
    editTaskQuery = vm::editTaskQuery,
    pickTask = vm::pickTask,
    createTask = vm::createTask,
)

/** The plan's actions; as on Home, a started timer makes its task Focus's task. */
internal fun planActions(vm: PlanViewModel, sheet: TaskSheetViewModel, focus: FocusViewModel, timerStarted: () -> Unit = {}, edgeSpeed: (Float, Float, Float) -> Float = RustPlanRules::edgeSpeed) = PlanActions(
    toggleTask = vm::toggleTask,
    openTask = sheet::open,
    startTimer = { taskId ->
        if (vm.startTimer(taskId)) {
            focus.rememberTask(taskId)
            timerStarted()
        }
    },
    stopTimer = vm::stopTimer,
    deleteTask = vm::deleteTask,
    undo = vm::undo,
    dismissToast = vm::dismissToast,
    openFocus = focus::open,
    editDraft = vm::editDraft,
    addTask = vm::addTask,
    setAddPosition = vm::setAddPosition,
    setQuestFilter = vm::setQuestFilter,
    setShowOther = vm::setShowOther,
    visibleRange = vm::onVisibleRange,
    jumpTo = vm::jumpTo,
    scrolled = vm::scrolled,
    moveTask = vm::moveTask,
    nudgeTask = vm::nudgeTask,
    shiftTask = vm::shiftTask,
    openMoveTo = vm::openMoveTo,
    moveTo = vm::moveTo,
    closeMoveTo = vm::closeMoveTo,
    edgeSpeed = edgeSpeed,
    assignQuest = vm::assignQuest,
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

/** Home's actions; starting a timer also makes its task Focus's task and asks for notifications once. */
internal fun homeActions(vm: HomeViewModel, sheet: TaskSheetViewModel, focus: FocusViewModel, timerStarted: () -> Unit = {}) = HomeActions(
    toggleTask = vm::toggleTask,
    openTask = sheet::open,
    startTimer = { taskId ->
        if (vm.startTimer(taskId)) {
            focus.rememberTask(taskId)
            timerStarted()
        }
    },
    stopTimer = vm::stopTimer,
    deleteTask = vm::deleteTask,
    undo = vm::undo,
    dismissToast = vm::dismissToast,
    editDraft = vm::editDraft,
    addTask = vm::addTask,
    setQuestFilter = vm::setQuestFilter,
    pickQuest = vm::pickQuest,
    setShowOther = vm::setShowOther,
    setAddPosition = vm::setAddPosition,
    setCompactView = vm::setCompactView,
    openFocus = focus::open,
    assignQuest = vm::assignQuest,
)

internal fun taskSheetActions(vm: TaskSheetViewModel) = TaskSheetActions(
    close = vm::close,
    editTitle = vm::editTitle,
    commitTitle = vm::commitTitle,
    editDescription = vm::editDescription,
    commitDescription = vm::commitDescription,
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
)

/** Where a detail page can go: another detail, back, and what search's results do. */
private class DetailNavigation(
    val open: (Detail) -> Unit,
    val back: () -> Unit,
    val search: SearchNavigation,
    val timerStarted: () -> Unit,
    val captureShown: Boolean,
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
        Detail.Archive -> scaffolded { ArchiveDetail(viewModels, snackbar) }
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
