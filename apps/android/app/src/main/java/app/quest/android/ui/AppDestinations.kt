package app.quest.android.ui

import app.quest.android.capture.CaptureActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.quest.android.R
import app.quest.android.settings.Appearance
import app.quest.android.ui.archive.ArchiveActions
import app.quest.android.ui.archive.ArchiveScreen
import app.quest.android.ui.components.QuestSnackbarHost
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.lists.ListActions
import app.quest.android.ui.lists.QuickAddActions
import app.quest.android.ui.lists.ListScreen
import app.quest.android.ui.lists.ListUiState
import app.quest.android.ui.lists.ListViewModel
import app.quest.android.ui.lists.RowActions
import app.quest.android.ui.quests.QuestEditorActions
import app.quest.android.ui.quests.QuestEditorSheet
import app.quest.android.ui.quests.QuestEditorViewModel
import app.quest.android.ui.quests.QuestNavActions
import app.quest.android.ui.quests.QuestNavScreen
import app.quest.android.ui.review.ReviewScreen
import app.quest.android.ui.search.SearchActions
import app.quest.android.ui.search.SearchScreen
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.tracked.TrackedActions
import app.quest.android.ui.tracked.TrackedScreen
import uniffi.quest_mobile.AppView
import uniffi.quest_mobile.ListKind
import uniffi.quest_mobile.SearchOutcome

/** One list (Today, Inbox, the Logbook, a quest's); the core reads it only while it is composed. */
@Composable
internal fun ListDestination(
    kind: ListKind,
    viewModels: AppViewModels,
    snackbar: SnackbarHostState,
    timerStarted: () -> Unit,
    onGone: () -> Unit = {},
    showQuickAdd: Boolean = true,
    headerActions: @Composable () -> Unit = {},
) {
    val vm = viewModels.list
    DisposableEffect(vm, kind) {
        vm.show(kind)
        onDispose { vm.show(null) }
    }
    val state by vm.state.collectAsStateWithLifecycle()
    ShowMessages(vm.message, vm::dismissMessage, snackbar)
    LaunchedEffect(state) { if (state == ListUiState.Gone) onGone() }
    val focus = viewModels.focus
    val rows = RowActions(
        toggle = vm::toggleTask,
        open = viewModels.taskSheet::open,
        startTimer = { taskId ->
            if (vm.startTimer(taskId)) {
                focus.open(taskId)
                timerStarted()
            }
        },
        stopTimer = vm::stopTimer,
    )
    val quickError by vm.quickError.collectAsStateWithLifecycle()
    ListScreen(
        state, vm.listActions(rows), headerActions = headerActions, showQuickAdd = showQuickAdd,
        quickError = quickError, dismissQuickError = vm::dismissQuickError,
    )
}

internal fun ListViewModel.listActions(rows: RowActions) = ListActions(
    rows, ::editDraft, ::add, ::undo, ::dismissToast,
    QuickAddActions(::questOptions, ::suggestions, ::setQuest, ::setDate, ::calendar, ::shiftMonth, ::editNote),
    ::placeTask,
)

/** "‹ Quests" above a quest's list or the Logbook. */
@Composable
internal fun BackToQuests(onBack: () -> Unit) {
    TextButton(onClick = onBack, modifier = Modifier.testTag("quests-back")) {
        Icon(Icons.AutoMirrored.Filled.KeyboardArrowLeft, contentDescription = null, tint = QuestTheme.colors.textMuted)
        Text(stringResource(R.string.nav_quests), color = QuestTheme.colors.textMuted)
    }
}

/** The edit button on a quest's list: the quest editor (rename, colour, complete, delete). */
@Composable
internal fun EditQuestButton(onClick: () -> Unit) {
    IconButton(onClick = onClick, modifier = Modifier.testTag("quest-edit")) {
        Icon(Icons.Filled.Edit, contentDescription = stringResource(R.string.quest_edit), tint = QuestTheme.colors.textMuted)
    }
}

/** The Quests tab; the core reads it only while it is composed. */
@Composable
internal fun QuestNavDestination(viewModels: AppViewModels, openQuest: (String) -> Unit, openLogbook: () -> Unit) {
    val vm = viewModels.questNav
    DisposableEffect(vm) {
        vm.setShown(true)
        onDispose { vm.setShown(false) }
    }
    val state by vm.state.collectAsStateWithLifecycle()
    QuestNavScreen(state, QuestNavActions(openQuest = openQuest, newQuest = viewModels.questEditor::openNew, openLogbook = openLogbook))
}

/** The quest editor over the Quests tab. */
@Composable
internal fun QuestEditor(viewModels: AppViewModels, snackbar: SnackbarHostState, openQuest: (String) -> Unit) {
    val editor = viewModels.questEditor
    val editorState by editor.state.collectAsStateWithLifecycle()
    ShowMessages(editor.message, editor::dismissMessage, snackbar)
    QuestEditorSheet(editorState, questEditorActions(editor, openQuest))
}

internal fun questEditorActions(editor: QuestEditorViewModel, openQuest: (String) -> Unit) = QuestEditorActions(
    close = editor::close,
    editTitle = editor::editTitle,
    titleDone = editor::titleDone,
    commitTitle = editor::commitTitle,
    setHue = editor::setHue,
    create = editor::create,
    toggleStatus = editor::toggleStatus,
    openPage = { editor.openPage()?.let(openQuest) },
    delete = editor::delete,
)

/** The Tracked tab; the core reads it only while it is composed. */
@Composable
internal fun TrackedDestination(viewModels: AppViewModels, openReview: () -> Unit) {
    val vm = viewModels.tracked
    DisposableEffect(vm) {
        vm.setShown(true)
        onDispose { vm.setShown(false) }
    }
    val state by vm.state.collectAsStateWithLifecycle()
    TrackedScreen(state, TrackedActions(openEntry = viewModels.entry::open, openReview = openReview, logWork = viewModels.logWork::open))
}

/** The weekly review (from Tracked or Search); the core builds it only while it is composed. */
@Composable
internal fun ReviewDestination(viewModels: AppViewModels, snackbar: SnackbarHostState) {
    val vm = viewModels.review
    DisposableEffect(vm) {
        vm.setShown(true)
        onDispose { vm.setShown(false) }
    }
    val state by vm.state.collectAsStateWithLifecycle()
    val player = viewModels.player
    val toast by player.toast.collectAsStateWithLifecycle()
    ShowMessages(vm.message, vm::dismissMessage, snackbar)
    ReviewScreen(state, vm.actions(viewModels.taskSheet::open), footer = {
        toast?.let { UndoSnackbar(it.text, player::undo, actionLabel = it.actionLabel, onDismiss = player::dismissToast) }
    })
}

/** The archive (from the drawer or Search); read only while composed. */
@Composable
internal fun ArchiveDetail(viewModels: AppViewModels, snackbar: SnackbarHostState, openQuest: (String) -> Unit) {
    val vm = viewModels.archive
    DisposableEffect(vm) {
        vm.setShown(true)
        onDispose { vm.setShown(false) }
    }
    val state by vm.state.collectAsStateWithLifecycle()
    ShowMessages(vm.message, vm::dismissMessage, snackbar)
    ArchiveScreen(state, ArchiveActions(restore = vm::restore, unarchive = vm::unarchive, openQuest = openQuest))
}

/** Where a search result leads; [QuestApp] supplies the navigation. */
internal class SearchNavigation(
    val close: () -> Unit,
    /** A place of the app: a tab, the plan, the review, Focus or the archive. */
    val show: (AppView) -> Unit,
    val openQuest: (String) -> Unit,
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

/** Search closes, then the outcome happens: a place, a quest's list, Focus, Log work, theme, Account, Manage workspaces. */
private fun carryOutSearch(outcome: SearchOutcome, viewModels: AppViewModels, timerStarted: () -> Unit, navigation: SearchNavigation) {
    navigation.close()
    when (outcome) {
        is SearchOutcome.Show -> navigation.show(outcome.view)
        is SearchOutcome.ShowQuest -> navigation.openQuest(outcome.questId)
        is SearchOutcome.Focus -> {
            viewModels.focus.open(outcome.taskId)
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
