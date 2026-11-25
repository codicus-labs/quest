package app.quest.android.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.DayGridCard
import app.quest.android.ui.components.HoursHeader
import app.quest.android.ui.components.QuestIcons
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
    val setCompactView: (CompactView) -> Unit,
    /** Opens Focus on a task (its running time was tapped). */
    val openFocus: (taskId: String) -> Unit,
    /** Gives a task without a quest one (the card's "Assign quest" icon). */
    val assignQuest: (taskId: String, questId: String) -> Unit = { _, _ -> },
) {
    companion object {
        /** Does nothing; for previews and screenshots. */
        val None = HomeActions(
            {}, {}, {}, {}, {}, {}, {}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {},
        )
    }
}

/** From this width Home shows the list and the day grid side by side (medium and expanded). */
private val SideBySideWidth = 600.dp

/** The widest the task list grows next to the grid. */
private val ListMaxWidth = 720.dp

/** Today: Quest Hours, QUEST WORK and OTHER, the day grid, and the undo snackbar. */
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
        if (maxWidth >= SideBySideWidth) {
            Row(Modifier.fillMaxSize().padding(end = QuestSpacing.screen), horizontalArrangement = Arrangement.spacedBy(QuestSpacing.screen)) {
                HomeList(state, actions, header = { Header(state) }, modifier = Modifier.weight(1.15f).widthIn(max = ListMaxWidth))
                DayGridCard(state.grid, Modifier.weight(1f).fillMaxSize().padding(vertical = QuestSpacing.small))
            }
            return@BoxWithConstraints
        }
        val header = @Composable { Header(state, compact = true) { CompactSwitch(state.inputs.compactView, actions.setCompactView) } }
        when (state.inputs.compactView) {
            CompactView.List -> HomeList(state, actions, header = header, modifier = Modifier.fillMaxSize())
            CompactView.Grid -> Column(
                Modifier.fillMaxSize().padding(horizontal = QuestSpacing.screen),
                verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
            ) {
                Column(Modifier.padding(top = QuestSpacing.small), verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) { header() }
                DayGridCard(state.grid, Modifier.weight(1f).fillMaxWidth().padding(bottom = QuestSpacing.small))
            }
        }
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
private fun Header(state: HomeUiState.Ready, compact: Boolean = false, trailing: @Composable () -> Unit = {}) {
    val home = state.home
    SyncPill(state.sync)
    HoursHeader(home.dayLabel, home.todayHours, home.weekHours, home.weekProgress, weeklyTarget = home.weeklyTarget, compact = compact, trailing = trailing)
}

/** Phones: switches between the list and the day grid; the icon shows the other view. */
@Composable
private fun CompactSwitch(view: CompactView, onChange: (CompactView) -> Unit) {
    val showsList = view == CompactView.List
    IconButton(
        onClick = { onChange(if (showsList) CompactView.Grid else CompactView.List) },
        modifier = Modifier.testTag("home-view-switch"),
    ) {
        Icon(
            if (showsList) QuestIcons.DayGridView else QuestIcons.ListView,
            contentDescription = stringResource(if (showsList) R.string.home_view_grid else R.string.home_view_list),
            tint = QuestTheme.colors.textMuted,
        )
    }
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
