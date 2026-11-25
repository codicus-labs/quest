package app.quest.android.screenshots

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.quest.android.ListSamples
import app.quest.android.ui.liftAndDrag
import app.quest.android.LogWorkSamples
import app.quest.android.Samples
import app.quest.android.WorkspaceSamples
import app.quest.android.ui.BackToQuests
import app.quest.android.ui.EditQuestButton
import app.quest.android.ui.lists.ListActions
import app.quest.android.ui.lists.ListScreen
import app.quest.android.ui.lists.ListUiState
import app.quest.android.ui.lists.RowActions
import app.quest.android.ui.logwork.TaskPickerState
import app.quest.android.ui.navigation.QuestScaffold
import app.quest.android.ui.navigation.Tab
import app.quest.android.ui.quests.QuestNavActions
import app.quest.android.ui.quests.QuestNavScreen
import app.quest.android.ui.quests.QuestNavUiState
import app.quest.android.ui.shell.AccountDrawerContent
import app.quest.android.ui.shell.DrawerActions
import app.quest.android.ui.shell.DrawerData
import app.quest.android.ui.shell.MiniPlayer
import app.quest.android.ui.shell.PlayerActions
import app.quest.android.ui.shell.TodaySegment
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.tracked.EntryActions
import app.quest.android.ui.tracked.EntryForm
import app.quest.android.ui.tracked.EntrySheetContent
import app.quest.android.ui.tracked.EntrySheetState
import app.quest.android.ui.tracked.TrackedActions
import app.quest.android.ui.tracked.TrackedScreen
import app.quest.android.ui.tracked.TrackedUiState
import app.quest.android.ui.workspaces.TimerElsewhereBar
import uniffi.quest_mobile.PlayerCard
import uniffi.quest_mobile.SyncState
import uniffi.quest_mobile.TaskList
import uniffi.quest_mobile.WorkspacesData

private val noListActions = ListActions(RowActions({}, {}, {}, {}), {}, {}, {}, {})
private val noQuestNavActions = QuestNavActions({}, {}, {})

/** The app frame as [app.quest.android.ui.QuestApp] draws it: tabs, the top row, the player. */
@Composable
internal fun Shell(
    tab: Tab,
    player: PlayerCard = PlayerCard.Empty,
    workspaces: WorkspacesData = WorkspaceSamples.one,
    leading: @Composable () -> Unit = {},
    abovePlayer: @Composable () -> Unit = {},
    content: @Composable () -> Unit,
) = QuestScaffold(
    tab = tab,
    onTab = {},
    onSearch = {},
    onAccount = {},
    workspaceName = workspaces.active.name,
    leading = leading,
    timerNote = { TimerElsewhereBar(workspaces, onClick = {}) },
    player = {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            abovePlayer()
            MiniPlayer(player, PlayerActions.None)
        }
    },
    content = content,
)

@Composable
private fun TodayList(list: TaskList, player: PlayerCard, draft: String = "") =
    Shell(Tab.Today, player, leading = { TodaySegment(showPlan = false, onChange = {}) }) { ListScreen(ListUiState.Ready(list, draft), noListActions) }

@Composable
private fun QuestsList(list: TaskList, editable: Boolean) = Shell(Tab.Quests, ListSamples.playerResume, leading = { BackToQuests {} }) {
    ListScreen(ListUiState.Ready(list, ""), noListActions, headerActions = { if (editable) EditQuestButton {} })
}

private val entryForm = EntryForm.of(ListSamples.entryDraft)

/** The redesigned shell: lists, Quests, Tracked, the drawer, the entry sheet and the mini player. */
val shellScenes = listOf(
    Scene("today-sort-drag", SceneKind.Screen, interact = {
        liftAndDrag("row:Draft this week’s essay", "row:Sketch a pricing page")
    }) { TodayList(ListSamples.todayIdle, PlayerCard.Empty) },
    Scene("today-timer", SceneKind.Screen) { TodayList(ListSamples.today, ListSamples.playerRunning) },
    Scene("today-add-draft", SceneKind.Screen) { TodayList(ListSamples.todayIdle, ListSamples.playerResume, draft = "Book the train") },
    Scene("player-organizing", SceneKind.Screen) { TodayList(ListSamples.todayIdle, ListSamples.playerOrganizing) },
    Scene("inbox", SceneKind.Screen) { Shell(Tab.Inbox, ListSamples.playerRunning) { ListScreen(ListUiState.Ready(ListSamples.inbox, ""), noListActions) } },
    Scene("inbox-empty", SceneKind.Screen) { Shell(Tab.Inbox) { ListScreen(ListUiState.Ready(ListSamples.emptyInbox, ""), noListActions) } },
    Scene("list-toast", SceneKind.Screen) {
        Shell(Tab.Inbox) { ListScreen(ListUiState.Ready(ListSamples.inbox.copy(toast = Samples.toast), ""), noListActions) }
    },
    Scene("quests", SceneKind.Screen) { Shell(Tab.Quests, ListSamples.playerResume) { QuestNavScreen(QuestNavUiState.Ready(ListSamples.questNav), noQuestNavActions) } },
    Scene("quest-list", SceneKind.Screen) { QuestsList(ListSamples.questList, editable = true) },
    Scene("logbook", SceneKind.Screen) { QuestsList(ListSamples.logbook, editable = false) },
    Scene("tracked", SceneKind.Screen) {
        Shell(Tab.Tracked, ListSamples.playerRunning) { TrackedScreen(TrackedUiState.Ready(ListSamples.tracked), TrackedActions({}, {}, {})) }
    },
    Scene("account-drawer", SceneKind.Screen) {
        SheetFrame { AccountDrawerContent(DrawerData(WorkspaceSamples.two, Samples.account, SyncState.Live(0u)), DrawerActions.None) }
    },
    Scene("account-drawer-guest", SceneKind.Screen) {
        SheetFrame { AccountDrawerContent(DrawerData(WorkspaceSamples.one, null, SyncState.SignedOut), DrawerActions.None) }
    },
    Scene("entry-sheet", SceneKind.Screen) {
        SheetFrame { EntrySheetContent(EntrySheetState.Fields(entryForm, ListSamples.entryCheck, null), EntryActions.None) }
    },
    Scene("entry-sheet-organizing", SceneKind.Screen) {
        SheetFrame { EntrySheetContent(EntrySheetState.Fields(entryForm.copy(taskId = null), ListSamples.organizingCheck, null), EntryActions.None) }
    },
    Scene("entry-sheet-task-picker", SceneKind.Screen) {
        Box(Modifier.fillMaxSize().background(QuestTheme.colors.card).padding(top = 24.dp)) {
            EntrySheetContent(EntrySheetState.PickingTask(TaskPickerState("", LogWorkSamples.choices)), EntryActions.None)
        }
    },
    Scene("mini-player", SceneKind.Component) {
        MiniPlayer(ListSamples.playerRunning, PlayerActions.None)
        MiniPlayer(ListSamples.playerOrganizing, PlayerActions.None)
        MiniPlayer(ListSamples.playerResume, PlayerActions.None)
    },
    Scene("workspace-timer-elsewhere", SceneKind.Screen) {
        Shell(Tab.Quests, workspaces = WorkspaceSamples.timerInDefault) { QuestNavScreen(QuestNavUiState.Ready(ListSamples.questNav), noQuestNavActions) }
    },
)
