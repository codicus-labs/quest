package app.quest.android.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.HoursHeader
import app.quest.android.ui.components.SyncPill
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.TaskSection

/** Everything Home can do, wired to [HomeViewModel] (and the task sheet for [openTask]). */
class HomeActions(
    val toggleTask: (String) -> Unit,
    val openTask: (String) -> Unit,
    val startTimer: (String) -> Unit,
    val stopTimer: () -> Unit,
    val deleteTask: (String) -> Unit,
    val undo: () -> Unit,
    val dismissToast: () -> Unit,
    val editDraft: (TaskSection, String) -> Unit,
    val addTask: (TaskSection) -> Unit,
    val setQuestFilter: (String?) -> Unit,
    val pickQuest: (String) -> Unit,
    val setShowOther: (Boolean) -> Unit,
    val setAddPosition: (AddPosition) -> Unit,
    /** Opens Focus on a task (its running time was tapped). */
    val openFocus: (taskId: String) -> Unit,
    /** Gives a task without a quest one (the card's "Assign quest" icon). */
    val assignQuest: (taskId: String, questId: String) -> Unit = { _, _ -> },
) {
    companion object {
        /** Does nothing; for previews and screenshots. */
        val None = HomeActions(
            {}, {}, {}, {}, {}, {}, {}, { _, _ -> }, {}, {}, {}, {}, {}, {},
        )
    }
}

/** Below this width (phones) Home's header is compact. */
private val CompactWidth = 600.dp

/** The widest the task list grows (tablets); centred beyond. */
private val ListMaxWidth = 720.dp

/** Today: Quest Hours, QUEST WORK and OTHER, and the undo snackbar. */
@Composable
fun HomeScreen(state: HomeUiState, actions: HomeActions) {
    when (state) {
        HomeUiState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is HomeUiState.Failed -> CenteredNote(state.message)
        is HomeUiState.Ready -> Box(Modifier.fillMaxSize()) {
            HomeLayout(state, actions)
            state.home.toast?.let { toast ->
                UndoSnackbar(
                    toast.text,
                    onUndo = actions.undo,
                    actionLabel = toast.actionLabel,
                    onDismiss = actions.dismissToast,
                    modifier = Modifier.align(Alignment.BottomCenter).widthIn(max = 560.dp),
                )
            }
        }
    }
}

@Composable
private fun HomeLayout(state: HomeUiState.Ready, actions: HomeActions) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val compact = maxWidth < CompactWidth
        HomeList(
            state,
            actions,
            header = { Header(state, compact) },
            modifier = Modifier.fillMaxSize().wrapContentWidth(Alignment.CenterHorizontally).widthIn(max = ListMaxWidth),
        )
    }
}

@Composable
private fun HomeList(state: HomeUiState.Ready, actions: HomeActions, header: @Composable () -> Unit, modifier: Modifier) {
    LazyColumn(
        modifier = modifier.testTag("home"),
        // Room at the bottom so the undo snackbar does not cover the last card.
        contentPadding = PaddingValues(start = QuestSpacing.screen, end = QuestSpacing.screen, top = QuestSpacing.small, bottom = 88.dp),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.listGap),
    ) {
        item(key = "header") {
            Column(Modifier.padding(bottom = QuestSpacing.small), verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) { header() }
        }
        questWorkSection(state, actions)
        otherSection(state, actions)
    }
}

@Composable
private fun Header(state: HomeUiState.Ready, compact: Boolean) {
    val home = state.home
    SyncPill(state.sync)
    HoursHeader(home.dayLabel, home.todayHours, home.weekHours, home.weekProgress, weeklyTarget = home.weeklyTarget, compact = compact)
}

@Composable
fun EmptyNote(message: String) {
    Box(Modifier.fillMaxWidth().padding(vertical = 18.dp), contentAlignment = Alignment.Center) {
        Text(message, style = QuestType.body, color = QuestTheme.colors.textFaint)
    }
}

@Composable
fun CenteredNote(message: String) {
    Box(Modifier.fillMaxSize().padding(QuestSpacing.screen), contentAlignment = Alignment.Center) {
        Text(message, style = QuestType.body, color = QuestTheme.colors.textMuted)
    }
}
