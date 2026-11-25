package app.quest.android.ui.home

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestChip
import app.quest.android.ui.components.QuestFilterButton
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.QuestPickerChip
import app.quest.android.ui.components.SectionHeader
import app.quest.android.ui.components.TaskCardActions
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.HomeTask
import uniffi.quest_mobile.QuestChip as QuestRef
import uniffi.quest_mobile.TaskSection

/**
 * QUEST WORK as on the desktop: the header with the quest filter (its chip clears it, the icon
 * opens the quest picker), the add field with its quest chip, and the cards.
 */
fun LazyListScope.questWorkSection(state: HomeUiState.Ready, actions: HomeActions) {
    val home = state.home
    item(key = "quest-work-header") {
        SectionHeader(stringResource(R.string.home_quest_work)) {
            home.questFilter?.let { quest -> FilterChipButton(quest) { actions.setQuestFilter(null) } }
            if (home.quests.isNotEmpty()) QuestFilterButton(home.questFilter, home.quests, actions.setQuestFilter, "quest-filter")
        }
    }
    item(key = "quest-work-add") {
        AddField(
            section = TaskSection.QUEST_WORK,
            text = state.inputs.questWorkDraft,
            placeholder = stringResource(R.string.home_add_quest_work),
            position = state.prefs.addPosition,
            actions = actions,
            questChip = home.addQuest?.let { quest -> { AddQuestChip(quest, home.quests, actions.pickQuest) } },
        )
    }
    if (home.questWork.isEmpty()) {
        item(key = "quest-work-empty") { EmptyNote(stringResource(R.string.home_quest_work_empty)) }
    }
    cards(home.questWork, state, actions)
}

/** OTHER: a header with show/hide, then (when shown) its add field and cards. */
fun LazyListScope.otherSection(state: HomeUiState.Ready, actions: HomeActions) {
    val showOther = state.prefs.showOther
    item(key = "other-header") {
        SectionHeader(stringResource(R.string.home_other), Modifier.padding(top = QuestSpacing.large)) {
            val label = stringResource(if (showOther) R.string.home_hide_other else R.string.home_show_other)
            IconButton(onClick = { actions.setShowOther(!showOther) }, modifier = Modifier.testTag("home-other-toggle")) {
                Icon(if (showOther) QuestIcons.EyeOff else QuestIcons.Eye, contentDescription = label, tint = QuestTheme.colors.textMuted)
            }
        }
    }
    if (!showOther) return
    item(key = "other-add") {
        AddField(
            section = TaskSection.OTHER,
            text = state.inputs.otherDraft,
            placeholder = stringResource(R.string.home_add_other),
            position = state.prefs.addPosition,
            actions = actions,
            questChip = null,
        )
    }
    cards(state.home.other, state, actions)
}

private fun LazyListScope.cards(tasks: List<HomeTask>, state: HomeUiState.Ready, actions: HomeActions) {
    items(tasks, key = { it.id }) { task ->
        SwipeTaskCard(
            task = task,
            actions = TaskCardActions(
                onToggle = { actions.toggleTask(task.id) },
                onOpen = { actions.openTask(task.id) },
                onTimer = { if (task.timerRunning) actions.stopTimer() else actions.startTimer(task.id) },
                onFocus = { actions.openFocus(task.id) },
                onAssignQuest = { questId -> actions.assignQuest(task.id, questId) },
                questOptions = state.home.quests,
            ),
            onDelete = { actions.deleteTask(task.id) },
        )
    }
}

/** The filter's chip in the header; tapping it clears the filter (desktop). */
@Composable
private fun FilterChipButton(quest: QuestRef, onClear: () -> Unit) {
    val label = stringResource(R.string.plan_filter_clear)
    Box(
        Modifier
            .heightIn(min = QuestSpacing.touchTarget)
            .widthIn(max = 180.dp)
            .testTag("quest-filter-chip")
            .clickable(onClick = onClear)
            .clearAndSetSemantics { contentDescription = "$label: ${quest.name}"; role = Role.Button },
        contentAlignment = Alignment.Center,
    ) { QuestChip(quest) }
}

/**
 * The add field, one line: the quest chip inside it (quest work only; tap to pick), the title,
 * and where the task goes (top or bottom; tap the icon to switch). The keyboard's Done or the
 * add button adds it.
 */
@Composable
private fun AddField(
    section: TaskSection,
    text: String,
    placeholder: String,
    position: AddPosition,
    actions: HomeActions,
    questChip: (@Composable () -> Unit)?,
) {
    val tag = if (section == TaskSection.QUEST_WORK) "add-quest-work" else "add-other"
    OutlinedTextField(
        value = text,
        onValueChange = { actions.editDraft(section, it.replace('\n', ' ')) },
        modifier = Modifier.fillMaxWidth().testTag(tag),
        placeholder = { Text(placeholder, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        leadingIcon = questChip?.let { chip -> { Box(Modifier.padding(start = QuestSpacing.tight).widthIn(max = 150.dp)) { chip() } } },
        trailingIcon = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                PositionToggle(section, position, actions.setAddPosition)
                if (text.isNotBlank()) {
                    IconButton(onClick = { actions.addTask(section) }, modifier = Modifier.testTag("$tag-send")) {
                        Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.home_add_task), tint = QuestTheme.colors.accent)
                    }
                }
            }
        },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { actions.addTask(section) }),
    )
}

/** Where new tasks go (desktop Top/Bottom): a small icon in the field; tapping it switches. */
@Composable
private fun PositionToggle(section: TaskSection, position: AddPosition, onChange: (AddPosition) -> Unit) {
    val top = position == AddPosition.TOP
    val state = stringResource(if (top) R.string.home_add_top else R.string.home_add_bottom)
    val label = stringResource(R.string.home_add_position)
    IconButton(
        onClick = { onChange(if (top) AddPosition.BOTTOM else AddPosition.TOP) },
        modifier = Modifier
            .semantics {
                contentDescription = label
                stateDescription = state
                role = Role.Switch
            }
            .testTag("add-position-${section.name.lowercase()}"),
    ) {
        Icon(if (top) QuestIcons.ToTop else QuestIcons.ToBottom, contentDescription = null, tint = QuestTheme.colors.textMuted, modifier = Modifier.size(20.dp))
    }
}

/** The quest new quest work gets; tap to pick another (desktop rule: filter, else this chip). */
@Composable
private fun AddQuestChip(quest: QuestRef, quests: List<QuestRef>, onPick: (String) -> Unit) {
    QuestPickerChip(quest, quests, { picked -> picked?.let { onPick(it.id) } }, stringResource(R.string.home_add_quest), "add-quest-chip", noneLabel = null)
}
