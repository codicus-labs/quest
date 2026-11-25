package app.quest.android.ui.quests

import androidx.compose.ui.semantics.heading
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.components.figureMinWidth
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.QuestsData

class QuestsActions(
    val selectTab: (QuestsTab) -> Unit,
    /** Opens the quest's editor over the tab. */
    val openQuest: (questId: String) -> Unit,
    val openNewQuest: () -> Unit,
    val openArchive: () -> Unit,
    val undo: () -> Unit,
    val dismissToast: () -> Unit,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = QuestsActions({}, {}, {}, {}, {}, {})
    }
}

/** Where cards go side by side: two or more columns from this width. */
private val CardMinWidth = 300.dp

/**
 * The Quests tab (the desktop's Quests page): subtitle, the three figures, Active/Completed with
 * Archive beside them, and the cards (a grid on wide windows) with a dashed "New quest" card after
 * the active ones. "New quest" is also a FAB on phones and a button in the header on wider windows.
 * A card opens the quest's editor as a sheet over the tab, which keeps its tab and scroll position.
 */
@Composable
fun QuestsScreen(state: QuestsUiState, actions: QuestsActions, gridState: LazyGridState = rememberLazyGridState()) {
    when (state) {
        QuestsUiState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is QuestsUiState.Failed -> Box(Modifier.fillMaxSize().padding(QuestSpacing.screen), contentAlignment = Alignment.Center) {
            Text(state.message, style = QuestType.body, color = QuestTheme.colors.textMuted)
        }
        is QuestsUiState.Ready -> QuestsPage(state, actions, gridState)
    }
}

@Composable
private fun QuestsPage(state: QuestsUiState.Ready, actions: QuestsActions, gridState: LazyGridState) {
    val quests = state.quests
    val tab = state.inputs.tab
    val cards = if (tab == QuestsTab.Active) quests.active else quests.completed
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val compact = maxWidth < 600.dp
        LazyVerticalGrid(
            columns = GridCells.Adaptive(CardMinWidth),
            state = gridState,
            contentPadding = PaddingValues(start = QuestSpacing.screen, end = QuestSpacing.screen, bottom = 96.dp),
            horizontalArrangement = Arrangement.spacedBy(QuestSpacing.large),
            verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
            modifier = Modifier.fillMaxSize().testTag("quests-list"),
        ) {
            item(span = { GridItemSpan(maxLineSpan) }) { Header(compact, actions) }
            item(span = { GridItemSpan(maxLineSpan) }) { Figures(quests) }
            item(span = { GridItemSpan(maxLineSpan) }) { TabsRow(quests, tab, actions) }
            item(span = { GridItemSpan(maxLineSpan) }) {
                val hint = if (tab == QuestsTab.Active) R.string.quests_hint else R.string.quests_hint_completed
                Text(stringResource(hint), style = QuestType.body, color = QuestTheme.colors.textMuted)
            }
            if (cards.isEmpty()) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    val empty = if (tab == QuestsTab.Active) R.string.quests_empty else R.string.quests_empty_completed
                    Text(stringResource(empty), style = QuestType.body, color = QuestTheme.colors.textFaint, modifier = Modifier.testTag("quests-empty-${tab.name.lowercase()}"))
                }
            }
            items(cards, key = { it.id }) { card -> QuestCardView(card, onOpen = { actions.openQuest(card.id) }) }
            if (tab == QuestsTab.Active) item(key = "new-quest-card") { NewQuestCard(actions.openNewQuest) }
        }
        Column(
            Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(QuestSpacing.large),
            horizontalAlignment = Alignment.End,
            verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
        ) {
            quests.toast?.let { toast ->
                UndoSnackbar(
                    toast.text,
                    onUndo = actions.undo,
                    actionLabel = toast.actionLabel,
                    onDismiss = actions.dismissToast,
                    modifier = Modifier.align(Alignment.CenterHorizontally).widthIn(max = 560.dp),
                )
            }
            if (compact) {
                ExtendedFloatingActionButton(
                    onClick = actions.openNewQuest,
                    icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                    text = { Text(stringResource(R.string.quests_new)) },
                    containerColor = QuestTheme.colors.accent,
                    contentColor = QuestTheme.colors.card,
                    modifier = Modifier.testTag("quests-new"),
                )
            }
        }
    }
}

@Composable
private fun Header(compact: Boolean, actions: QuestsActions) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            // The top bar shows the workspace switcher, so the tab's title is here (as on Review).
            Text(stringResource(R.string.nav_quests), style = QuestType.stat, color = QuestTheme.colors.text, modifier = Modifier.semantics { heading() })
            Text(stringResource(R.string.quests_subtitle), style = QuestType.body, color = QuestTheme.colors.textMuted)
        }
        if (!compact) {
            Button(
                onClick = actions.openNewQuest,
                colors = ButtonDefaults.buttonColors(containerColor = QuestTheme.colors.accent, contentColor = QuestTheme.colors.card),
                modifier = Modifier.padding(start = QuestSpacing.large).testTag("quests-new"),
            ) {
                Icon(Icons.Filled.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                Text(stringResource(R.string.quests_new), modifier = Modifier.padding(start = QuestSpacing.small))
            }
        }
    }
}

/** Quest hours · all time, In progress, Quests completed, in one bordered strip. */
@Composable
private fun Figures(quests: QuestsData) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    // Large text wraps the figures onto more lines instead of breaking their labels.
    FlowRow(Modifier.fillMaxWidth().background(colors.card, shape).border(QuestSpacing.hairline, colors.border, shape)) {
        val width = Modifier.widthIn(min = figureMinWidth()).weight(1f)
        Figure(stringResource(R.string.quests_all_time), quests.allTimeHours, "quests-all-time", width)
        Figure(stringResource(R.string.quests_in_progress), quests.inProgress, "quests-in-progress", width)
        Figure(stringResource(R.string.quests_completed_count), quests.completedCount, "quests-completed", width, trophy = true)
    }
}

@Composable
private fun Figure(label: String, value: String, tag: String, modifier: Modifier, trophy: Boolean = false) {
    val colors = QuestTheme.colors
    Column(modifier.padding(QuestSpacing.large).semantics(mergeDescendants = true) {}, verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            if (trophy) Icon(QuestIcons.Trophy, contentDescription = null, tint = colors.accent, modifier = Modifier.size(14.dp))
            Text(label, style = QuestType.small, color = colors.textMuted, maxLines = 2)
        }
        Text(value, style = QuestType.stat, color = colors.text, modifier = Modifier.testTag(tag))
    }
}

/** Active · n / Completed · n as a segmented choice, and Archive (no count) beside them, as on the desktop. */
@Composable
private fun TabsRow(quests: QuestsData, tab: QuestsTab, actions: QuestsActions) {
    // Archive wraps below the tabs when both don't fit.
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, itemVerticalAlignment = Alignment.CenterVertically) {
        SingleChoiceSegmentedButtonRow {
            val labels = listOf(QuestsTab.Active to quests.activeLabel, QuestsTab.Completed to quests.completedLabel)
            labels.forEachIndexed { index, (option, label) ->
                SegmentedButton(
                    selected = option == tab,
                    onClick = { actions.selectTab(option) },
                    shape = SegmentedButtonDefaults.itemShape(index, labels.size),
                    icon = {},
                    colors = SegmentedButtonDefaults.colors(activeContainerColor = QuestTheme.colors.selected),
                    modifier = Modifier.testTag("quests-tab-${option.name.lowercase()}"),
                ) { Text(label, style = QuestType.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis) }
            }
        }
        OutlinedButton(onClick = actions.openArchive, modifier = Modifier.testTag("quests-archive")) {
            Icon(QuestIcons.Archive, contentDescription = null, tint = QuestTheme.colors.textMuted, modifier = Modifier.size(18.dp))
            Text(stringResource(R.string.archive), style = QuestType.bodyMedium, color = QuestTheme.colors.text, modifier = Modifier.padding(start = QuestSpacing.small))
        }
    }
}

/** The dashed "+ New quest" card after the active quests. */
@Composable
private fun NewQuestCard(onClick: () -> Unit) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    val dash = PathEffect.dashPathEffect(floatArrayOf(12f, 9f))
    Column(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 120.dp)
            .clip(shape)
            .drawBehind {
                val radius = QuestSpacing.cardRadius.toPx()
                drawRoundRect(colors.borderStrong, cornerRadius = CornerRadius(radius), style = Stroke(width = 1.dp.toPx(), pathEffect = dash))
            }
            .clickable(role = Role.Button, onClick = onClick)
            .testTag("quests-new-card"),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.small, Alignment.CenterVertically),
    ) {
        Icon(Icons.Filled.Add, contentDescription = null, tint = colors.textMuted)
        Text(stringResource(R.string.quests_new), style = QuestType.body, color = colors.textMuted)
    }
}
