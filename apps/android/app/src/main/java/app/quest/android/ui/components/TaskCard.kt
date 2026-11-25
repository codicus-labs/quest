package app.quest.android.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.HomeTask
import uniffi.quest_mobile.QuestChip as QuestRef

private val AccentBarWidth = 3.dp

/** What a card can do; [TaskCard] wires each to its control. */
class TaskCardActions(
    val onToggle: () -> Unit,
    val onOpen: () -> Unit,
    /** Starts the timer, or pauses it when it runs. */
    val onTimer: () -> Unit,
    /** Opens Focus on this task (tap the running time). */
    val onFocus: () -> Unit = {},
    /** Gives a task without a quest one of [questOptions] (its "Assign quest" icon). */
    val onAssignQuest: (questId: String) -> Unit = {},
    val questOptions: List<QuestRef> = emptyList(),
)

/**
 * A task as on the desktop: quest colour bar, title, complete toggle, quest chip, tracked time
 * and play/pause. Tapping the card, title included, opens the task's sheet, where the title is edited.
 */
@Composable
fun TaskCard(task: HomeTask, actions: TaskCardActions, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val accent = task.quest?.let { questColor(it.colorHex) } ?: colors.textFaint
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    val edge = if (task.timerRunning) colors.accent.copy(alpha = 0.7f) else colors.border
    Row(
        modifier = modifier
            .fillMaxWidth()
            .then(if (task.timerRunning) Modifier.shadow(8.dp, shape, spotColor = colors.accent) else Modifier)
            .alpha(if (task.done) 0.75f else 1f)
            .background(colors.card, shape)
            .border(QuestSpacing.hairline, edge, shape)
            .clickable(onClickLabel = stringResource(R.string.task_open), onClick = actions.onOpen)
            // The quest colour bar; drawn, not laid out, so the card needs no intrinsic height.
            .drawBehind {
                val inset = QuestSpacing.medium.toPx()
                drawRoundRect(
                    accent.copy(alpha = if (task.done) 0.45f else 1f),
                    topLeft = Offset(QuestSpacing.small.toPx(), inset),
                    size = Size(AccentBarWidth.toPx(), (size.height - 2 * inset).coerceAtLeast(0f)),
                    cornerRadius = CornerRadius(2.dp.toPx()),
                )
            }
            .padding(start = QuestSpacing.small + AccentBarWidth)
            .testTag("task:${task.title}"),
    ) {
        Column(Modifier.weight(1f).padding(start = QuestSpacing.cardPadding, top = QuestSpacing.tight, bottom = 4.dp)) {
            CardTitle(task)
            // Wraps the time and timer to a second line before it shortens the quest's name.
            FlowRow(itemVerticalAlignment = Alignment.CenterVertically) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    CompleteToggle(task.done, accent, actions.onToggle, size = QuestSpacing.cardControl)
                    if (task.quest != null) {
                        QuestChip(task.quest, Modifier.padding(start = QuestSpacing.small, end = QuestSpacing.medium))
                    } else {
                        AssignQuestButton(task.title, actions.questOptions, actions.onAssignQuest)
                    }
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    TrackedTime(task, accent, actions.onFocus)
                    if (task.canStartTimer) TimerButton(task.title, task.timerRunning, actions.onTimer)
                }
            }
        }
    }
}

@Composable
private fun CardTitle(task: HomeTask) {
    val colors = QuestTheme.colors
    Text(
        task.title,
        style = QuestType.task,
        color = if (task.done) colors.textFaint else colors.text,
        textDecoration = if (task.done) TextDecoration.LineThrough else null,
        modifier = Modifier
            .fillMaxWidth()
            .padding(end = QuestSpacing.medium)
            .padding(vertical = 4.dp)
            .testTag("task-title:${task.title}"),
    )
}

/**
 * Tracked time like the desktop card ("36:00"), with a dot while the timer runs. While it runs,
 * tapping it opens Focus.
 */
@Composable
private fun TrackedTime(task: HomeTask, accent: Color, onFocus: () -> Unit, modifier: Modifier = Modifier) {
    val tracked = task.tracked ?: return
    val colors = QuestTheme.colors
    val open = if (task.timerRunning) {
        Modifier.clickable(onClickLabel = stringResource(R.string.focus_open), onClick = onFocus).testTag("task-focus:${task.title}")
    } else {
        Modifier
    }
    // Always a control high, so the row keeps its height when the timer starts or stops.
    Row(modifier.heightIn(min = QuestSpacing.cardControl).then(open), horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        if (task.timerRunning) Dot(accent, 7.dp)
        Text(tracked, style = QuestType.monoTime, color = if (task.done) colors.textFaint else colors.text, modifier = Modifier.testTag("task-tracked"))
    }
}

/** Without a quest: a quiet quest icon instead of a "No quest" chip; tap for the quest picker. */
@Composable
private fun AssignQuestButton(title: String, quests: List<QuestRef>, onAssign: (String) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    val label = stringResource(R.string.task_assign_quest)
    IconButton(
        onClick = { open = true },
        enabled = quests.isNotEmpty(),
        modifier = Modifier.size(QuestSpacing.cardControl).testTag("task-assign-quest:$title"),
    ) {
        Icon(QuestIcons.AssignQuest, contentDescription = label, tint = QuestTheme.colors.textFaint)
    }
    if (open) {
        QuestPickerSheet(label, quests, selectedId = null, noneLabel = null, onPick = { quest -> quest?.let { onAssign(it.id) } }, onDismiss = { open = false })
    }
}

@Composable
private fun TimerButton(title: String, running: Boolean, onClick: () -> Unit) {
    val label = stringResource(if (running) R.string.timer_pause else R.string.timer_start)
    // Sized below 48 dp, IconButton still keeps a 48 dp touch target around it.
    IconButton(onClick = onClick, modifier = Modifier.size(QuestSpacing.cardControl).testTag(if (running) "timer-pause:$title" else "timer-start:$title")) {
        Icon(if (running) QuestIcons.Pause else Icons.Filled.PlayArrow, contentDescription = label, tint = QuestTheme.colors.textMuted)
    }
}

/**
 * The round "complete" toggle: a 24 dp circle in a [size] box. Below 48 dp the touch target still
 * reaches 48 dp (`minimumInteractiveComponentSize`).
 */
@Composable
fun CompleteToggle(done: Boolean, color: Color, onToggle: () -> Unit, modifier: Modifier = Modifier, size: Dp = QuestSpacing.touchTarget) {
    val colors = QuestTheme.colors
    val (fill, edge, ink) =
        if (done) Triple(color.copy(alpha = 0.85f), color, Color.White) else Triple(Color.Transparent, colors.borderStrong, colors.textMuted)
    Box(
        modifier = modifier
            .size(size)
            .minimumInteractiveComponentSize()
            .toggleable(value = done, role = Role.Checkbox, onValueChange = { onToggle() })
            .testTag("task-check"),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier.size(QuestSpacing.checkSize).background(fill, CircleShape).border(QuestSpacing.hairline, edge, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(Icons.Filled.Check, contentDescription = stringResource(R.string.task_complete), tint = ink, modifier = Modifier.size(14.dp))
        }
    }
}
