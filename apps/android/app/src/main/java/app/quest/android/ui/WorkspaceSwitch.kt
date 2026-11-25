package app.quest.android.ui

import app.quest.android.ui.quests.QuestDetailUiState

/**
 * What a workspace switch resets, as on the desktop (`quest-app` `state/workspaces.rs`): open sheets
 * save into the workspace they belong to and close (task, quest editor, quest page, Log work, entry);
 * Home's and the plan's quest filter and pick go, and Focus forgets its task. The tab, Search,
 * Account, Settings and the archive page stay; typed "Add …" text stays and lands in the workspace
 * then shown. The core drops its undo; the focus timer is one per person and keeps running.
 *
 * Called before a switch made here (so drafts save into their own workspace) and again once the
 * core shows another workspace (also when another device deleted the shown one). Returns whether a
 * quest's page was open: the app then shows Quests instead.
 */
internal fun AppViewModels.leaveWorkspace(): Boolean {
    val questPageOpen = questDetail.state.value != QuestDetailUiState.Hidden
    taskSheet.close()
    questEditor.close()
    if (questPageOpen) questDetail.close()
    calendar.closeSheet()
    home.leaveWorkspace()
    plan.setQuestFilter(null)
    focus.leaveWorkspace()
    return questPageOpen
}
