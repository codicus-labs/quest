package app.quest.android.ui.quests

import app.quest.android.ui.components.figureMinWidth
import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import app.quest.android.R
import app.quest.android.ui.components.InlineTitleField
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.components.questColor
import app.quest.android.ui.task.Caps
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import kotlin.coroutines.cancellation.CancellationException
import uniffi.quest_mobile.HueOption
import uniffi.quest_mobile.QuestDetail
import uniffi.quest_mobile.QuestHue

class QuestDetailActions(
    val close: () -> Unit,
    /** Turns the name into a field in its place (tap on the name). */
    val beginRename: () -> Unit,
    val editTitle: (String) -> Unit,
    val commitTitle: () -> Unit,
    val saveDrafts: () -> Unit,
    val setHue: (QuestHue) -> Unit,
    /** Complete quest, Reopen quest or Unarchive. */
    val toggleStatus: () -> Unit,
    val delete: () -> Unit,
    val undo: () -> Unit,
    val dismissToast: () -> Unit,
    val tasks: QuestTaskActions,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = QuestDetailActions({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, QuestTaskActions.None)
    }
}

/** From this width tasks and milestones sit side by side. */
private val TwoColumnWidth = 840.dp

/** Widest the page's content gets (Material's readable width on large screens); centred beyond. */
private val ContentMaxWidth = 840.dp

/**
 * A quest's page over the tabs (back, also predictive, closes it), the working view of its tasks as
 * on the desktop: name (tap to rename) with its status badge, colour, figures, Complete/Reopen quest
 * (Delete in the top bar), then its tasks and, if it has any, its milestones to read (side by side
 * when wide). Description and the rest are in the quest editor on the Quests tab.
 */
@Composable
fun QuestDetailScreen(state: QuestDetailUiState, actions: QuestDetailActions) {
    if (state == QuestDetailUiState.Hidden) return
    var backProgress by remember { mutableFloatStateOf(0f) }
    PredictiveBackHandler { events ->
        try {
            events.collect { backProgress = it.progress }
            actions.close()
        } catch (e: CancellationException) {
            backProgress = 0f
            throw e
        }
    }
    // Swiping the app from Recents may end the process without another chance to save.
    LifecycleEventEffect(Lifecycle.Event.ON_STOP) { actions.saveDrafts() }
    Box(
        Modifier
            .fillMaxSize()
            .background(QuestTheme.colors.bg)
            .graphicsLayer {
                val scale = 1f - 0.08f * backProgress
                scaleX = scale
                scaleY = scale
            }
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .testTag("quest-detail"),
    ) {
        when (state) {
            QuestDetailUiState.Hidden -> Unit
            QuestDetailUiState.Loading -> Column {
                TopBar(actions.close, onDelete = null)
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            }
            is QuestDetailUiState.Failed -> Column {
                TopBar(actions.close, onDelete = null)
                Text(state.message, style = QuestType.body, color = QuestTheme.colors.textMuted, modifier = Modifier.padding(QuestSpacing.screen))
            }
            is QuestDetailUiState.Ready -> DetailContent(state, actions)
        }
    }
}

@Composable
private fun DetailContent(state: QuestDetailUiState.Ready, actions: QuestDetailActions) {
    val detail = state.detail
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val wide = maxWidth >= TwoColumnWidth
        Column(Modifier.fillMaxSize()) {
            TopBar(actions.close, onDelete = actions.delete)
            Column(
                Modifier
                    .weight(1f)
                    .imePadding()
                    .verticalScroll(rememberScrollState())
                    .fillMaxWidth()
                    .wrapContentWidth(Alignment.CenterHorizontally)
                    .widthIn(max = ContentMaxWidth)
                    .padding(horizontal = QuestSpacing.screen)
                    .padding(bottom = 96.dp),
                verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
            ) {
                QuestName(state, actions)
                Caps(stringResource(R.string.quest_color))
                HuePicker(detail.hues, actions.setHue)
                Figures(detail)
                StatusRow(detail, actions)
                val milestones = detail.milestones
                if (wide && milestones.isNotEmpty()) {
                    Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.section)) {
                        Column(Modifier.weight(1.3f), verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) { QuestTasks(state, actions.tasks) }
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) { Milestones(milestones, detail.colorHex) }
                    }
                } else {
                    QuestTasks(state, actions.tasks)
                    if (milestones.isNotEmpty()) Milestones(milestones, detail.colorHex)
                }
            }
        }
        detail.toast?.let { toast ->
            UndoSnackbar(
                toast.text,
                onUndo = actions.undo,
                actionLabel = toast.actionLabel,
                onDismiss = actions.dismissToast,
                modifier = Modifier.align(Alignment.BottomCenter).padding(QuestSpacing.large).widthIn(max = 560.dp),
            )
        }
    }
}

/**
 * The name as the page's heading; tapping it renames it in place: the field has the heading's style and
 * place (desktop `heading`), so nothing moves or resizes. A "Completed" or "Archived" badge follows it.
 */
@Composable
private fun QuestName(state: QuestDetailUiState.Ready, actions: QuestDetailActions) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
        Box(Modifier.weight(1f)) { NameOrField(state, actions) }
        state.detail.badge?.let { Badge(it) }
    }
}

@Composable
private fun Badge(text: String) {
    val colors = QuestTheme.colors
    Text(
        text.uppercase(),
        style = QuestType.caps,
        color = colors.textMuted,
        modifier = Modifier
            .border(QuestSpacing.hairline, colors.borderStrong, RoundedCornerShape(6.dp))
            .padding(horizontal = 10.dp, vertical = 4.dp)
            .testTag("quest-badge"),
    )
}

@Composable
private fun NameOrField(state: QuestDetailUiState.Ready, actions: QuestDetailActions) {
    // Shared by text and field, so both sit in the same box; at least a touch target high.
    val place = Modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget)
    val renameLabel = stringResource(R.string.quest_rename)
    if (state.renamingTitle) {
        InlineTitleField(
            value = state.title,
            onEdit = actions.editTitle,
            onCommit = actions.commitTitle,
            style = QuestType.heading,
            placeholder = stringResource(R.string.quest_name),
            modifier = place.testTag("quest-rename"),
        )
    } else {
        Text(
            state.title,
            style = QuestType.heading,
            color = QuestTheme.colors.text,
            modifier = Modifier
                .semantics { heading() }
                .then(place)
                .clickable(onClickLabel = renameLabel, onClick = actions.beginRename)
                .testTag("quest-title"),
        )
    }
}

@Composable
private fun TopBar(onBack: () -> Unit, onDelete: (() -> Unit)?) {
    val colors = QuestTheme.colors
    Row(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = onBack, modifier = Modifier.testTag("quest-back")) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back), tint = colors.textMuted)
        }
        Text(stringResource(R.string.nav_quests), style = QuestType.caps, color = colors.textMuted, modifier = Modifier.weight(1f))
        if (onDelete != null) {
            IconButton(onClick = onDelete, modifier = Modifier.testTag("quest-delete")) {
                Icon(QuestIcons.Trash, contentDescription = stringResource(R.string.quest_delete), tint = colors.textMuted)
            }
        }
    }
}

/** The nine colours as a radio group of circles; each a 48 dp target named by its colour. */
@Composable
internal fun HuePicker(hues: List<HueOption>, onPick: (QuestHue) -> Unit, tag: String = "quest-hues") {
    val colors = QuestTheme.colors
    FlowRow(Modifier.selectableGroup().testTag(tag)) {
        hues.forEach { option ->
            Box(
                Modifier
                    .size(QuestSpacing.touchTarget)
                    .selectable(selected = option.selected, role = Role.RadioButton, onClick = { onPick(option.hue) })
                    .semantics { contentDescription = option.name }
                    .testTag("quest-hue:${option.name}"),
                contentAlignment = Alignment.Center,
            ) {
                Box(
                    Modifier
                        .size(32.dp)
                        .then(if (option.selected) Modifier.border(2.dp, colors.accent, CircleShape) else Modifier)
                        .padding(if (option.selected) 4.dp else 0.dp)
                        .background(questColor(option.colorHex), CircleShape),
                )
            }
        }
    }
}

/** This week, total, open and done, in one bordered strip. */
@Composable
private fun Figures(detail: QuestDetail) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    // Large text wraps the figures onto more lines instead of cutting their labels.
    FlowRow(Modifier.fillMaxWidth().background(colors.card, shape).border(QuestSpacing.hairline, colors.border, shape)) {
        val width = Modifier.widthIn(min = figureMinWidth()).weight(1f)
        Figure(stringResource(R.string.quest_week_hours), detail.weekHours, "quest-week-hours", width)
        Figure(stringResource(R.string.quest_total_hours), detail.totalHours, "quest-total-hours", width)
        Figure(stringResource(R.string.quest_open_count), detail.openCount.toString(), "quest-open-count", width)
        Figure(stringResource(R.string.quest_done_count), detail.doneCount.toString(), "quest-done-count", width)
    }
}

@Composable
private fun Figure(label: String, value: String, tag: String, modifier: Modifier) {
    Column(modifier.padding(QuestSpacing.medium).semantics(mergeDescendants = true) {}) {
        Text(label, style = QuestType.small, color = QuestTheme.colors.textMuted, maxLines = 2)
        Text(value, style = QuestType.stat, color = QuestTheme.colors.text, modifier = Modifier.testTag(tag))
    }
}

/** "Complete quest", "Reopen quest" or "Unarchive" (the core's label); Delete is in the top bar. */
@Composable
private fun StatusRow(detail: QuestDetail, actions: QuestDetailActions) {
    OutlinedButton(onClick = actions.toggleStatus, modifier = Modifier.testTag("quest-status")) {
        // Check only on "Complete quest": a completed or archived quest has its badge.
        if (detail.badge == null) Icon(Icons.Filled.Check, contentDescription = null, modifier = Modifier.size(18.dp))
        Text(detail.statusAction, style = QuestType.bodyMedium, modifier = Modifier.padding(start = QuestSpacing.small))
    }
}
