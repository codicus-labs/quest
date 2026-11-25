package app.quest.android.screenshots

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.quest.android.QuestSamples
import app.quest.android.Samples
import app.quest.android.WorkspaceSamples
import app.quest.android.ui.archive.ArchiveActions
import app.quest.android.ui.archive.ArchiveScreen
import app.quest.android.ui.archive.ArchiveUiState
import app.quest.android.ui.components.QuestSheetContent
import app.quest.android.ui.navigation.DetailScaffold
import app.quest.android.ui.navigation.QuestScaffold
import app.quest.android.ui.navigation.TopDestination
import app.quest.android.ui.quests.QuestEditorActions
import app.quest.android.ui.quests.QuestEditorFields
import app.quest.android.ui.quests.QuestEditorState
import app.quest.android.ui.quests.QuestDetailActions
import app.quest.android.ui.quests.QuestDetailInputs
import app.quest.android.ui.quests.QuestDetailScreen
import app.quest.android.ui.quests.QuestDetailUiState
import app.quest.android.ui.quests.QuestsActions
import app.quest.android.ui.quests.QuestsInputs
import app.quest.android.ui.quests.QuestsScreen
import app.quest.android.ui.quests.QuestsTab
import app.quest.android.ui.quests.QuestsUiState
import app.quest.android.ui.review.ReviewActions
import app.quest.android.ui.review.ReviewScreen
import app.quest.android.ui.review.ReviewUiState
import app.quest.android.ui.search.SavedSearchMode
import app.quest.android.ui.search.SearchActions
import app.quest.android.ui.search.SearchScreen
import app.quest.android.ui.search.SearchUiState
import app.quest.android.ui.theme.QuestTheme
import uniffi.quest_mobile.QuestsData
import uniffi.quest_mobile.SearchKind
import uniffi.quest_mobile.SearchResult
import uniffi.quest_mobile.SearchToken

@Composable
private fun Tab(destination: TopDestination, content: @Composable () -> Unit) =
    QuestScaffold(destination, onNavigate = {}, onSearch = {}, onAccount = {}, workspaces = WorkspaceSamples.one, content = content)

@Composable
private fun Quests(quests: QuestsData = QuestSamples.quests, inputs: QuestsInputs = QuestsInputs()) =
    Tab(TopDestination.Quests) { QuestsScreen(QuestsUiState.Ready(quests, inputs), QuestsActions.None) }

private fun detail(inputs: QuestDetailInputs = QuestDetailInputs(questId = "q1"), detail: uniffi.quest_mobile.QuestDetail = QuestSamples.detail) =
    QuestDetailUiState.Ready(detail, inputs)

private val moveChoices = listOf(
    SearchResult("Today", "Mon, Nov 23", SearchKind.CHOICE, SearchToken.MoveTask("t1", "2026-11-23")),
    SearchResult("Tomorrow", "Tue, Nov 24", SearchKind.CHOICE, SearchToken.MoveTask("t1", "2026-11-24")),
    SearchResult("Backlog", "", SearchKind.CHOICE, SearchToken.MoveTask("t1", null)),
)

/** Quests, the quest page, review, archive and search. */
val questScenes = listOf(
    Scene("quests", SceneKind.Screen) { Quests() },
    Scene("quests-completed", SceneKind.Screen) { Quests(inputs = QuestsInputs(tab = QuestsTab.Completed)) },
    Scene("quests-empty", SceneKind.Screen) { Quests(QuestSamples.emptyQuests) },
    Scene("quests-toast", SceneKind.Screen) { Quests(QuestSamples.quests.copy(toast = Samples.toast.copy(text = "Deleted “Old hobby”"))) },
    // The quest editor over the tab (the desktop's "New quest" / "Edit quest" dialog).
    Scene("quest-editor-new", SceneKind.Screen) {
        val new = QuestEditorState.New("", "", QuestSamples.quests.newQuestHues, "Enter a name.")
        SheetFrame { QuestSheetContent("New quest") { QuestEditorFields(new, QuestEditorActions.None, autoFocus = false) } }
    },
    Scene("quest-editor", SceneKind.Screen) {
        val editing = QuestEditorState.Editing(QuestSamples.detail, QuestSamples.detail.title, QuestSamples.detail.description)
        SheetFrame { QuestSheetContent("Edit quest") { QuestEditorFields(editing, QuestEditorActions.None, autoFocus = false) } }
    },
    Scene("quest-editor-completed", SceneKind.Screen) {
        val completed = QuestSamples.toggled(QuestSamples.detail)
        SheetFrame { QuestSheetContent("Edit quest") { QuestEditorFields(QuestEditorState.Editing(completed, completed.title, ""), QuestEditorActions.None, autoFocus = false) } }
    },
    Scene("quest-detail", SceneKind.Screen) { QuestDetailScreen(detail(), QuestDetailActions.None) },
    Scene("quest-detail-editing", SceneKind.Screen) {
        val inputs = QuestDetailInputs(questId = "q1", taskDraft = "Write the epilogue", toBacklog = true)
        QuestDetailScreen(detail(inputs), QuestDetailActions.None)
    },
    // The name renamed in place: same type and place as in "quest-detail".
    Scene("quest-detail-renaming", SceneKind.Screen) {
        QuestDetailScreen(detail(QuestDetailInputs(questId = "q1", title = "Write the book")), QuestDetailActions.None)
    },
    Scene("quest-detail-no-milestones", SceneKind.Screen) {
        QuestDetailScreen(detail(detail = QuestSamples.detail.copy(milestones = emptyList())), QuestDetailActions.None)
    },
    Scene("quest-detail-completed", SceneKind.Screen) {
        val completed = QuestSamples.toggled(QuestSamples.detail).copy(taskGroups = emptyList(), milestones = emptyList(), openCount = 0u, toast = Samples.toast.copy(text = "Completed quest “Write the book”"))
        QuestDetailScreen(detail(detail = completed), QuestDetailActions.None)
    },
    Scene("review", SceneKind.Screen) { Tab(TopDestination.Review) { ReviewScreen(ReviewUiState.Ready(QuestSamples.review), ReviewActions.None) } },
    Scene("review-other-week", SceneKind.Screen) {
        val empty = QuestSamples.lastWeek.copy(byQuest = emptyList(), questHours = "0h", dailyAverage = "0h", questShare = "0%", targetProgress = 0f, questFraction = 0f)
        Tab(TopDestination.Review) { ReviewScreen(ReviewUiState.Ready(empty), ReviewActions.None) }
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
