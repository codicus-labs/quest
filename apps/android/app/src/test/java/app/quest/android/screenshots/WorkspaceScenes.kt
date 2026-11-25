package app.quest.android.screenshots

import app.quest.android.Samples
import app.quest.android.WorkspaceSamples
import app.quest.android.ui.components.QuestSheetContent
import app.quest.android.ui.focus.FocusActions
import app.quest.android.ui.focus.FocusScreen
import app.quest.android.ui.focus.FocusUiState
import app.quest.android.ui.workspaces.DeleteQuestion
import app.quest.android.ui.workspaces.ManageActions
import app.quest.android.ui.workspaces.ManageInputs
import app.quest.android.ui.workspaces.ManageWorkspacesFields
import app.quest.android.ui.workspaces.TimerElsewhereNote

private const val DELETE_WORK = "Delete “Work”? It holds 0 quests, 1 task, 0h tracked. You can restore it under Deleted."

val workspaceScenes = listOf(
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
