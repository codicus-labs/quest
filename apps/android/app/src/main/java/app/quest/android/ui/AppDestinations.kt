package app.quest.android.ui

import app.quest.android.capture.CaptureActivity
import androidx.compose.ui.platform.LocalContext
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.quest.android.settings.Appearance
import app.quest.android.ui.archive.ArchiveActions
import app.quest.android.ui.archive.ArchiveScreen
import app.quest.android.ui.components.QuestSnackbarHost
import app.quest.android.ui.focus.FocusViewModel
import app.quest.android.ui.navigation.TopDestination
import app.quest.android.ui.quests.QuestEditorActions
import app.quest.android.ui.quests.QuestEditorSheet
import app.quest.android.ui.quests.QuestEditorViewModel
import app.quest.android.ui.quests.QuestDetailActions
import app.quest.android.ui.quests.QuestDetailScreen
import app.quest.android.ui.quests.QuestDetailViewModel
import app.quest.android.ui.quests.QuestTaskActions
import app.quest.android.ui.quests.QuestsActions
import app.quest.android.ui.quests.QuestsScreen
import app.quest.android.ui.review.ReviewActions
import app.quest.android.ui.review.ReviewScreen
import app.quest.android.ui.search.SearchActions
import app.quest.android.ui.search.SearchScreen
import app.quest.android.ui.task.TaskSheet
import uniffi.quest_mobile.AppView
import uniffi.quest_mobile.SearchOutcome

/** The Quests tab; the core builds its cards only while it is composed. */
@Composable
internal fun QuestsDestination(viewModels: AppViewModels, snackbar: SnackbarHostState, openArchive: () -> Unit) {
    val vm = viewModels.quests
    DisposableEffect(vm) {
        vm.setShown(true)
        onDispose { vm.setShown(false) }
    }
    val state by vm.state.collectAsStateWithLifecycle()
    ShowMessages(vm.message, vm::dismissMessage, snackbar)
    val editor = viewModels.questEditor
    val editorState by editor.state.collectAsStateWithLifecycle()
    ShowMessages(editor.message, editor::dismissMessage, snackbar)
    QuestsScreen(
        state,
        QuestsActions(
            selectTab = vm::selectTab,
            openQuest = editor::open,
            openNewQuest = editor::openNew,
            openArchive = openArchive,
            undo = vm::undo,
            dismissToast = vm::dismissToast,
        ),
    )
    QuestEditorSheet(editorState, questEditorActions(editor, viewModels.questDetail))
}

internal fun questEditorActions(editor: QuestEditorViewModel, page: QuestDetailViewModel) = QuestEditorActions(
    close = editor::close,
    editTitle = editor::editTitle,
    titleDone = editor::titleDone,
    commitTitle = editor::commitTitle,
    setHue = editor::setHue,
    create = editor::create,
    toggleStatus = editor::toggleStatus,
    openPage = { editor.openPage()?.let(page::open) },
    delete = editor::delete,
)

/** The weekly review; the core builds it only while it is composed. */
@Composable
internal fun ReviewDestination(viewModels: AppViewModels) {
    val vm = viewModels.review
    DisposableEffect(vm) {
        vm.setShown(true)
        onDispose { vm.setShown(false) }
    }
    val state by vm.state.collectAsStateWithLifecycle()
    ReviewScreen(state, ReviewActions(previous = { vm.previous() }, next = { vm.next() }, thisWeek = vm::thisWeek, selectWeek = vm::selectWeek))
}

/** The archive (from the Quests tab, Account or Search); read only while composed. */
@Composable
internal fun ArchiveDetail(viewModels: AppViewModels, snackbar: SnackbarHostState) {
    val vm = viewModels.archive
    DisposableEffect(vm) {
        vm.setShown(true)
        onDispose { vm.setShown(false) }
    }
    val state by vm.state.collectAsStateWithLifecycle()
    ShowMessages(vm.message, vm::dismissMessage, snackbar)
    ArchiveScreen(state, ArchiveActions(restore = vm::restore, unarchive = vm::unarchive, openQuest = viewModels.questDetail::open))
}

/** A quest's page over everything but Focus, with the task sheet for its tasks. */
@Composable
internal fun QuestDetailPage(viewModels: AppViewModels, snackbar: SnackbarHostState, timerStarted: () -> Unit) {
    val vm = viewModels.questDetail
    val state by vm.state.collectAsStateWithLifecycle()
    ShowMessages(vm.message, vm::dismissMessage, snackbar)
    val sheet = viewModels.taskSheet
    val sheetState by sheet.state.collectAsStateWithLifecycle()
    ShowMessages(sheet.message, sheet::dismissMessage, snackbar)
    Box {
        QuestDetailScreen(state, questDetailActions(vm, viewModels, timerStarted))
        QuestSnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).navigationBarsPadding())
    }
    TaskSheet(sheetState, taskSheetActions(sheet))
}

internal fun questDetailActions(vm: QuestDetailViewModel, viewModels: AppViewModels, timerStarted: () -> Unit) = QuestDetailActions(
    close = { vm.close() },
    beginRename = vm::beginRename,
    editTitle = vm::editTitle,
    commitTitle = vm::commitTitle,
    saveDrafts = vm::commitDrafts,
    setHue = { vm.setHue(it) },
    toggleStatus = { vm.toggleStatus() },
    delete = vm::delete,
    undo = { vm.undo() },
    dismissToast = { vm.dismissToast() },
    tasks = QuestTaskActions(
        toggle = { vm.toggleTask(it) },
        open = viewModels.taskSheet::open,
        startTimer = { taskId -> startTimer(vm, viewModels.focus, taskId, timerStarted) },
        stopTimer = { vm.stopTimer() },
        delete = { vm.deleteTask(it) },
        openFocus = { viewModels.focus.open(it) },
        editDraft = vm::editTaskDraft,
        setToBacklog = vm::setToBacklog,
        add = vm::addTask,
    ),
)

/** As on Home: a started timer makes its task Focus's task. */
private fun startTimer(vm: QuestDetailViewModel, focus: FocusViewModel, taskId: String, timerStarted: () -> Unit) {
    if (!vm.startTimer(taskId)) return
    focus.rememberTask(taskId)
    timerStarted()
}

/** Where a search result leads; [QuestApp] supplies the navigation. */
internal class SearchNavigation(
    val close: () -> Unit,
    val goTo: (TopDestination) -> Unit,
    val openArchive: () -> Unit,
    val openAccount: () -> Unit,
    val toggleTheme: () -> Unit,
    val openWorkspaces: () -> Unit,
)

@Composable
internal fun SearchPage(
    viewModels: AppViewModels,
    snackbar: SnackbarHostState,
    timerStarted: () -> Unit,
    navigation: SearchNavigation,
    captureShown: Boolean,
) {
    val context = LocalContext.current
    val vm = viewModels.search
    DisposableEffect(vm) {
        vm.setShown(true)
        onDispose { vm.setShown(false) }
    }
    val state by vm.state.collectAsStateWithLifecycle()
    ShowMessages(vm.message, vm::dismissMessage, snackbar)
    val carryOut = { outcome: SearchOutcome? -> outcome?.let { carryOutSearch(it, viewModels, timerStarted, navigation) } }
    Box {
        SearchScreen(
            state,
            SearchActions(
                close = {
                    vm.reset()
                    navigation.close()
                },
                editQuery = vm::editQuery,
                run = { carryOut(vm.run(it)) },
                runFirst = { carryOut(vm.runFirst()) },
                captureWithAi = if (captureShown) { query -> context.startActivity(CaptureActivity.intent(context, query)) } else null,
            ),
        )
        QuestSnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).navigationBarsPadding())
    }
}

/** Search closes, then the outcome happens: a tab, a quest's page, Focus, Log work, theme, Account, Manage workspaces. */
private fun carryOutSearch(outcome: SearchOutcome, viewModels: AppViewModels, timerStarted: () -> Unit, navigation: SearchNavigation) {
    navigation.close()
    when (outcome) {
        is SearchOutcome.Show -> when (outcome.view) {
            AppView.HOME -> navigation.goTo(TopDestination.Home)
            AppView.PLAN -> navigation.goTo(TopDestination.Plan)
            AppView.QUESTS -> navigation.goTo(TopDestination.Quests)
            AppView.REVIEW -> navigation.goTo(TopDestination.Review)
            AppView.FOCUS -> viewModels.focus.open()
            AppView.ARCHIVE -> navigation.openArchive()
        }
        is SearchOutcome.ShowQuest -> viewModels.questDetail.open(outcome.questId)
        is SearchOutcome.Focus -> {
            viewModels.focus.rememberTask(outcome.taskId)
            viewModels.focus.open()
            timerStarted()
        }
        // Handled by the ViewModel (the list switches); never reaches here.
        is SearchOutcome.Choose -> Unit
        SearchOutcome.ToggleTheme -> navigation.toggleTheme()
        SearchOutcome.Account -> navigation.openAccount()
        SearchOutcome.LogWork -> viewModels.logWork.open()
        SearchOutcome.ManageWorkspaces -> navigation.openWorkspaces()
        SearchOutcome.Done -> Unit
    }
}

/** "Toggle theme" flips what is shown now to the other fixed appearance. */
internal fun toggledAppearance(dark: Boolean): Appearance = if (dark) Appearance.Light else Appearance.Dark
