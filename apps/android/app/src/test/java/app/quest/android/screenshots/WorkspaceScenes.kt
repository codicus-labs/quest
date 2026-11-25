package app.quest.android.screenshots

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.quest.android.QuestSamples
import app.quest.android.Samples
import app.quest.android.WorkspaceSamples
import app.quest.android.ui.components.QuestSheetContent
import app.quest.android.ui.focus.FocusActions
import app.quest.android.ui.focus.FocusScreen
import app.quest.android.ui.focus.FocusUiState
import app.quest.android.ui.navigation.QuestScaffold
import app.quest.android.ui.navigation.TopDestination
import app.quest.android.ui.quests.QuestsActions
import app.quest.android.ui.quests.QuestsInputs
import app.quest.android.ui.quests.QuestsScreen
import app.quest.android.ui.quests.QuestsUiState
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.workspaces.DeleteQuestion
import app.quest.android.ui.workspaces.ManageActions
import app.quest.android.ui.workspaces.ManageInputs
import app.quest.android.ui.workspaces.ManageWorkspacesFields
import app.quest.android.ui.workspaces.SwitcherActions
import app.quest.android.ui.workspaces.TimerElsewhereNote
import app.quest.android.ui.workspaces.WorkspaceSwitcher

private const val DELETE_WORK = "Delete “Work”? It holds 0 quests, 1 task, 0h tracked. You can restore it under Deleted."

val workspaceScenes = listOf(
    Scene("workspace-timer-elsewhere", SceneKind.Screen) {
        QuestScaffold(TopDestination.Quests, onNavigate = {}, onSearch = {}, onAccount = {}, workspaces = WorkspaceSamples.timerInDefault) {
            QuestsScreen(QuestsUiState.Ready(QuestSamples.emptyQuests, QuestsInputs()), QuestsActions.None)
        }
    },
    Scene("workspace-menu", SceneKind.Window) {
        Box(Modifier.fillMaxSize().background(QuestTheme.colors.bg).padding(16.dp)) {
            WorkspaceSwitcher(WorkspaceSamples.two, SwitcherActions.None, startOpen = true)
        }
    },
    Scene("workspaces-manage", SceneKind.Screen) {
        SheetFrame { QuestSheetContent("Manage workspaces") { ManageWorkspacesFields(WorkspaceSamples.two, ManageInputs(), ManageActions.None) } }
    },
    Scene("workspaces-delete", SceneKind.Screen) {
        val manage = ManageInputs(confirmDelete = DeleteQuestion("w2", DELETE_WORK))
        SheetFrame { QuestSheetContent("Manage workspaces") { ManageWorkspacesFields(WorkspaceSamples.two, manage, ManageActions.None) } }
    },
    Scene("focus-timer-elsewhere", SceneKind.Screen) {
        FocusScreen(FocusUiState.Pick(Samples.pick.tasks), FocusActions.None, keepScreenOn = false) {
            TimerElsewhereNote(WorkspaceSamples.timerInDefault.timerElsewhere!!, onClick = {})
        }
    },
)
