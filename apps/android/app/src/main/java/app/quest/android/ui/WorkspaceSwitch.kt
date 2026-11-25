package app.quest.android.ui

/**
 * What a workspace switch resets, as on the desktop (`quest-app` `state/workspaces.rs`): open sheets
 * save into the workspace they belong to and close (task, quest editor, Log work, a tracked
 * session); Focus forgets its task. The tab, Search, Account,
 * Settings and the archive page stay; typed "New task" text stays and lands in the workspace then
 * shown. A quest's list gives way to the quests ([QuestApp]). The core drops its undo; the focus
 * timer is one per person and keeps running.
 *
 * Called before a switch made here (so drafts save into their own workspace) and again once the
 * core shows another workspace (also when another device deleted the shown one).
 */
internal fun AppViewModels.leaveWorkspace() {
    taskSheet.close()
    questEditor.close()
    logWork.close()
    entry.close()
    focus.leaveWorkspace()
}
