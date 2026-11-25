package app.quest.android.screenshots

import app.quest.android.QuestSamples
import app.quest.android.ui.archive.ArchiveActions
import app.quest.android.ui.archive.ArchiveScreen
import app.quest.android.ui.archive.ArchiveUiState
import app.quest.android.ui.components.QuestSheetContent
import app.quest.android.ui.navigation.DetailScaffold
import app.quest.android.ui.quests.QuestEditorActions
import app.quest.android.ui.quests.QuestEditorFields
import app.quest.android.ui.quests.QuestEditorState
import app.quest.android.ui.review.ReviewActions
import app.quest.android.ui.review.ReviewScreen
import app.quest.android.ui.review.ReviewUiState
import app.quest.android.ui.search.SavedSearchMode
import app.quest.android.ui.search.SearchActions
import app.quest.android.ui.search.SearchScreen
import app.quest.android.ui.search.SearchUiState
import uniffi.quest_mobile.SearchKind
import uniffi.quest_mobile.SearchResult
import uniffi.quest_mobile.SearchToken

private val moveChoices = listOf(
    SearchResult("Today", "Mon, Nov 23", SearchKind.CHOICE, SearchToken.MoveTask("t1", "2026-11-23")),
    SearchResult("Tomorrow", "Tue, Nov 24", SearchKind.CHOICE, SearchToken.MoveTask("t1", "2026-11-24")),
    SearchResult("Backlog", "", SearchKind.CHOICE, SearchToken.MoveTask("t1", null)),
)

/** The quest editor, review, archive and search. */
val questScenes = listOf(
    // The quest editor over the tab (the desktop's "New quest" / "Edit quest" dialog).
    Scene("quest-editor-new", SceneKind.Screen) {
        val new = QuestEditorState.New("", QuestSamples.quests.newQuestHues, "Enter a name.")
        SheetFrame { QuestSheetContent("New quest") { QuestEditorFields(new, QuestEditorActions.None, autoFocus = false) } }
    },
    Scene("quest-editor", SceneKind.Screen) {
        val editing = QuestEditorState.Editing(QuestSamples.detail, QuestSamples.detail.title)
        SheetFrame { QuestSheetContent("Edit quest") { QuestEditorFields(editing, QuestEditorActions.None, autoFocus = false) } }
    },
    Scene("quest-editor-completed", SceneKind.Screen) {
        val completed = QuestSamples.toggled(QuestSamples.detail)
        SheetFrame { QuestSheetContent("Edit quest") { QuestEditorFields(QuestEditorState.Editing(completed, completed.title), QuestEditorActions.None, autoFocus = false) } }
    },
    Scene("review", SceneKind.Screen) {
        DetailScaffold("Weekly review", onBack = {}) { ReviewScreen(ReviewUiState.Ready(QuestSamples.review), ReviewActions.None) }
    },
    Scene("review-previous", SceneKind.Screen) {
        DetailScaffold("Weekly review", onBack = {}) { ReviewScreen(ReviewUiState.Ready(QuestSamples.lastWeek), ReviewActions.None) }
    },
    Scene("archive", SceneKind.Screen) { DetailScaffold("Archive", onBack = {}) { ArchiveScreen(ArchiveUiState.Ready(QuestSamples.archive), ArchiveActions.None) } },
    Scene("archive-empty", SceneKind.Screen) { DetailScaffold("Archive", onBack = {}) { ArchiveScreen(ArchiveUiState.Ready(QuestSamples.emptyArchive), ArchiveActions.None) } },
    Scene("search", SceneKind.Screen) {
        SearchScreen(SearchUiState.Ready("", SavedSearchMode.Commands, "Search or jump…", QuestSamples.commands), SearchActions.None)
    },
    Scene("search-choice", SceneKind.Screen) {
        SearchScreen(SearchUiState.Ready("", SavedSearchMode.MoveTo("t1"), "Move “Outline chapter 3” to…", moveChoices), SearchActions.None)
    },
    Scene("search-empty", SceneKind.Screen) {
        SearchScreen(SearchUiState.Ready("zzz", SavedSearchMode.Commands, "Search or jump…", emptyList()), SearchActions.None)
    },
)
