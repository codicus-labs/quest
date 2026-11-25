package app.quest.android.ui.quests

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selectableGroup
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.SectionHeader
import app.quest.android.ui.components.TaskCardActions
import app.quest.android.ui.components.questColor
import app.quest.android.ui.home.SwipeTaskCard
import app.quest.android.ui.task.Caps
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.MilestoneRow

/** The quest's tasks: the cards act as on Home (check, timer, sheet, swipe). */
class QuestTaskActions(
    val toggle: (taskId: String) -> Unit,
    val open: (taskId: String) -> Unit,
    val startTimer: (taskId: String) -> Unit,
    val stopTimer: () -> Unit,
    val delete: (taskId: String) -> Unit,
    val openFocus: (taskId: String) -> Unit,
    val editDraft: (String) -> Unit,
    val setToBacklog: (Boolean) -> Unit,
    val add: () -> Unit,
) {
    companion object {
        val None = QuestTaskActions({}, {}, {}, {}, {}, {}, {}, {}, {})
    }
}

/** TASKS: the add field (today or backlog), then the core's groups (by day, Backlog, Done). */
@Composable
fun QuestTasks(state: QuestDetailUiState.Ready, actions: QuestTaskActions) {
    val inputs = state.inputs
    SectionHeader(stringResource(R.string.quest_tasks))
    AddField(
        text = inputs.taskDraft,
        placeholder = stringResource(R.string.quest_add_task),
        onEdit = actions.editDraft,
        onAdd = actions.add,
        tag = "quest-add-task",
    )
    Row(Modifier.semantics { selectableGroup() }, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        Text(stringResource(R.string.quest_add_to), style = QuestType.small, color = QuestTheme.colors.textMuted)
        TargetChip(stringResource(R.string.plan_today), selected = !inputs.toBacklog, "quest-add-today") { actions.setToBacklog(false) }
        TargetChip(stringResource(R.string.task_backlog), selected = inputs.toBacklog, "quest-add-backlog") { actions.setToBacklog(true) }
    }
    if (state.detail.taskGroups.isEmpty()) {
        Text(stringResource(R.string.quest_no_tasks), style = QuestType.body, color = QuestTheme.colors.textMuted)
    }
    state.detail.taskGroups.forEach { group ->
        Caps(group.label, Modifier.padding(top = QuestSpacing.small))
        group.tasks.forEach { task ->
            SwipeTaskCard(
                task = task,
                actions = TaskCardActions(
                    onToggle = { actions.toggle(task.id) },
                    onOpen = { actions.open(task.id) },
                    onTimer = { if (task.timerRunning) actions.stopTimer() else actions.startTimer(task.id) },
                    onFocus = { actions.openFocus(task.id) },
                ),
                onDelete = { actions.delete(task.id) },
            )
        }
    }
}

@Composable
private fun TargetChip(label: String, selected: Boolean, tag: String, onClick: () -> Unit) {
    FilterChip(
        selected = selected,
        onClick = onClick,
        label = { Text(label) },
        colors = FilterChipDefaults.filterChipColors(selectedContainerColor = QuestTheme.colors.selected),
        modifier = Modifier.testTag(tag),
    )
}

/** MILESTONES, newest first, to read only (the page shows this only when there are some). */
@Composable
fun Milestones(milestones: List<MilestoneRow>, colorHex: String) {
    val accent = questColor(colorHex)
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    SectionHeader(stringResource(R.string.quest_milestones), Modifier.testTag("quest-milestones"))
    milestones.forEach { milestone ->
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = QuestSpacing.touchTarget)
                .background(colors.card, shape)
                .border(QuestSpacing.hairline, colors.border, shape)
                .padding(horizontal = QuestSpacing.large, vertical = QuestSpacing.small)
                .testTag("milestone:${milestone.title}")
                .clearAndSetSemantics { contentDescription = milestone.a11yLabel },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
        ) {
            Icon(QuestIcons.Milestone, contentDescription = null, tint = accent, modifier = Modifier.size(14.dp))
            Text(milestone.title, style = QuestType.bodyMedium, color = colors.text, modifier = Modifier.weight(1f))
            Text(milestone.dateLabel, style = QuestType.small, color = colors.textMuted)
        }
    }
}

/** A one-line field that adds with the keyboard's action or its + button. */
@Composable
private fun AddField(text: String, placeholder: String, onEdit: (String) -> Unit, onAdd: () -> Unit, tag: String) {
    OutlinedTextField(
        value = text,
        onValueChange = onEdit,
        placeholder = { Text(placeholder, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        trailingIcon = {
            if (text.isNotBlank()) {
                IconButton(onClick = onAdd, modifier = Modifier.testTag("$tag-send")) {
                    Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.quest_add), tint = QuestTheme.colors.accent)
                }
            }
        },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { onAdd() }),
        modifier = Modifier.fillMaxWidth().testTag(tag),
    )
}
