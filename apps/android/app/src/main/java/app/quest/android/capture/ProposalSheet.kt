package app.quest.android.capture

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.logwork.DateField
import app.quest.android.ui.logwork.TimeField
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.QuestPickerChip
import uniffi.quest_mobile.QuestChip as QuestRef
import app.quest.android.ui.components.questColor
import app.quest.android.ui.task.Caps
import app.quest.android.ui.task.DayPickerDialog
import app.quest.android.ui.task.RepeatEditor
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue

/**
 * The proposal: new quests, tasks and time entries, each with its checkbox; tap one to edit it with the usual
 * pickers. Unsure guesses carry an icon and words. The core labels everything and says what blocks Create.
 */
@Composable
internal fun ProposalContent(state: CaptureUiState.Proposal, actions: ProposalActions) {
    val repeat = state.repeat
    BackHandler(enabled = repeat != null, onBack = actions.repeat.close)
    BackHandler(enabled = repeat == null, onBack = actions.backToText)
    if (repeat != null) {
        val task = state.draft.tasks.firstOrNull { it.key == state.expanded }
        return RepeatEditor(repeat, task?.title.orEmpty(), repeats = true, actions.repeat)
    }
    SheetTitle(stringResource(R.string.capture_proposal_title))
    val view = state.view
    val draft = state.draft
    if (draft.dropped.isNotEmpty()) Dropped(draft.dropped)
    if (draft.quests.isEmpty() && draft.tasks.isEmpty() && draft.work.isEmpty()) {
        Text(stringResource(R.string.capture_nothing_found), style = QuestType.body, color = QuestTheme.colors.textMuted)
    }
    if (view != null) {
        Group(R.string.capture_new_quests, draft.quests) { quest -> QuestItem(quest, view.quests.firstOrNull { it.key == quest.key }, state.expanded == quest.key, actions) }
        Group(R.string.capture_tasks, draft.tasks) { task ->
            TaskItem(task, view.tasks.firstOrNull { it.key == task.key }, view.questOptions, state.expanded == task.key, actions)
        }
        Group(R.string.capture_time_entries, draft.work) { work ->
            WorkItem(work, view.work.firstOrNull { it.key == work.key }, view.questOptions, state.expanded == work.key, actions)
        }
    }
    state.error?.let { Hint(it, "capture-error") }
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = actions.backToText, modifier = Modifier.testTag("capture-back")) { Text(stringResource(R.string.capture_back_to_text)) }
        Box(Modifier.weight(1f))
        Button(
            onClick = actions.apply,
            enabled = view?.canApply == true,
            modifier = Modifier.padding(start = QuestSpacing.small).testTag("capture-apply"),
        ) { Text(view?.applyLabel ?: stringResource(R.string.capture_structure)) }
    }
}

@Composable
private fun <T> Group(title: Int, items: List<T>, item: @Composable (T) -> Unit) {
    if (items.isEmpty()) return
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        Caps(stringResource(title), Modifier.semantics { heading() })
        items.forEach { item(it) }
    }
}

/** What the AI proposed but the core left out, as sentences. */
@Composable
private fun Dropped(sentences: List<String>) {
    Column(Modifier.testTag("capture-dropped"), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Caps(stringResource(R.string.capture_left_out))
        sentences.forEach { Text(it, style = QuestType.small, color = QuestTheme.colors.textMuted) }
    }
}

/**
 * One proposed item: checkbox, the summary (tap: open its fields), and when [expanded] the fields. [label] is the
 * item's name for TalkBack ("Include …", "Edit …").
 */
@Composable
private fun Item(
    key: String,
    label: String,
    selected: Boolean,
    onSelect: (Boolean) -> Unit,
    expanded: Boolean,
    onExpand: () -> Unit,
    summary: @Composable () -> Unit,
    fields: @Composable () -> Unit,
) {
    val colors = QuestTheme.colors
    Column(
        Modifier
            .fillMaxWidth()
            .border(QuestSpacing.hairline, if (expanded) colors.borderStrong else colors.border, RoundedCornerShape(8.dp))
            .testTag("capture-item:$key"),
    ) {
        Row(verticalAlignment = Alignment.Top) {
            val include = stringResource(R.string.capture_include, label)
            Box(
                Modifier
                    .size(QuestSpacing.touchTarget)
                    .toggleable(value = selected, role = Role.Checkbox, onValueChange = onSelect)
                    .semantics { contentDescription = include }
                    .testTag("capture-check:$key"),
                contentAlignment = Alignment.Center,
            ) { Checkbox(checked = selected, onCheckedChange = null) }
            val edit = stringResource(R.string.capture_edit, label)
            Column(
                Modifier
                    .weight(1f)
                    .heightIn(min = QuestSpacing.touchTarget)
                    .clickable(onClickLabel = edit, onClick = onExpand)
                    .padding(vertical = QuestSpacing.small)
                    .padding(end = QuestSpacing.medium)
                    .testTag("capture-open:$key"),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) { summary() }
        }
        if (expanded) {
            Column(
                Modifier.padding(start = QuestSpacing.medium, end = QuestSpacing.medium, bottom = QuestSpacing.medium),
                verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
            ) { fields() }
        }
    }
}

@Composable
private fun ItemTitle(title: String, selected: Boolean) {
    Text(
        title,
        style = QuestType.task,
        color = if (selected) QuestTheme.colors.text else QuestTheme.colors.textFaint,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
    )
}

@Composable
private fun Detail(text: String) {
    if (text.isNotEmpty()) Text(text, style = QuestType.small, color = QuestTheme.colors.textMuted)
}

@Composable
private fun ItemError(error: String?) {
    error?.let { Text(it, style = QuestType.small, color = QuestTheme.colors.accent, modifier = Modifier.testTag("capture-item-error")) }
}

/** An unsure guess: icon and words, not colour alone. */
@Composable
private fun Unsure(text: Int) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.testTag("capture-unsure")) {
        Icon(QuestIcons.Unsure, contentDescription = null, tint = QuestTheme.colors.accent, modifier = Modifier.size(16.dp))
        Text(stringResource(text), style = QuestType.smallMedium, color = QuestTheme.colors.accent, modifier = Modifier.padding(start = 4.dp))
    }
}

@Composable
private fun Chip(chip: CaptureChip?) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Dot(chip?.let { questColor(it.colorHex) } ?: QuestTheme.colors.textFaint, QuestSpacing.small)
        Text(
            chip?.name ?: stringResource(R.string.no_quest),
            style = QuestType.smallMedium,
            color = QuestTheme.colors.textMuted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(start = 4.dp),
        )
    }
}

@Composable
private fun QuestItem(quest: DraftQuest, view: DraftQuestView?, expanded: Boolean, actions: ProposalActions) {
    Item(
        quest.key, quest.title, quest.selected, { actions.setQuestSelected(quest.key, it) }, expanded, { actions.toggleExpanded(quest.key) },
        summary = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Dot(view?.let { questColor(it.colorHex) } ?: QuestTheme.colors.textFaint, QuestSpacing.small)
                Box(Modifier.padding(start = QuestSpacing.small)) { ItemTitle(quest.title, quest.selected) }
            }
            ItemError(view?.error)
        },
        fields = { TitleField(stringResource(R.string.capture_quest_title), quest.title, "capture-quest-title") { actions.editQuestTitle(quest.key, it) } },
    )
}

@Composable
private fun TaskItem(task: DraftTask, view: DraftTaskView?, options: List<DraftQuestOption>, expanded: Boolean, actions: ProposalActions) {
    Item(
        task.key, task.title, task.selected, { actions.setTaskSelected(task.key, it) }, expanded, { actions.toggleExpanded(task.key) },
        summary = {
            ItemTitle(task.title, task.selected)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Chip(view?.quest)
                Detail(view?.whenLabel.orEmpty())
            }
            view?.repeatSummary?.let { Row(verticalAlignment = Alignment.CenterVertically) { RepeatIcon(); Detail(it) } }
            if (task.questUncertain) Unsure(R.string.capture_unsure_quest)
            ItemError(view?.error)
        },
        fields = { TaskFields(task, options, actions) },
    )
}

@Composable
private fun RepeatIcon() = Icon(QuestIcons.Repeat, contentDescription = null, tint = QuestTheme.colors.textMuted, modifier = Modifier.size(16.dp).padding(end = 4.dp))

@Composable
private fun TaskFields(task: DraftTask, options: List<DraftQuestOption>, actions: ProposalActions) {
    val key = task.key
    TitleField(stringResource(R.string.task_title), task.title, "capture-task-title") { actions.editTaskTitle(key, it) }
    Caps(stringResource(R.string.task_quest))
    QuestChoice(options, task.quest, "capture-task-quest") { actions.setTaskQuest(key, it) }
    Caps(stringResource(R.string.task_date))
    TaskDay(task, actions)
    if (task.repeat != null) {
        TextButton(onClick = { actions.openRepeat(key) }, modifier = Modifier.testTag("capture-repeat")) {
            RepeatIcon()
            Text(stringResource(R.string.capture_repeat_edit))
        }
    }
}

/** The plan date (picker) or Someday. */
@Composable
private fun TaskDay(task: DraftTask, actions: ProposalActions) {
    var picking by rememberSaveable { mutableStateOf(false) }
    val day = task.plannedOn
    FlowRow(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small), modifier = Modifier.selectableGroup()) {
        FilterChip(
            selected = day != null,
            onClick = { picking = true },
            label = { Text(day?.label ?: stringResource(R.string.capture_pick_day)) },
            modifier = Modifier.semantics { role = Role.RadioButton }.testTag("capture-task-day"),
        )
        FilterChip(
            selected = day == null,
            onClick = { actions.setTaskDay(task.key, null) },
            label = { Text(stringResource(R.string.capture_someday)) },
            modifier = Modifier.semantics { role = Role.RadioButton }.testTag("capture-task-someday"),
        )
    }
    if (picking) DayPickerDialog(day?.iso, onPick = { actions.setTaskDay(task.key, it) }, onDismiss = { picking = false })
}

@Composable
private fun WorkItem(work: DraftWork, view: DraftWorkView?, options: List<DraftQuestOption>, expanded: Boolean, actions: ProposalActions) {
    val title = view?.taskTitle ?: workTitle(work.task)
    Item(
        work.key, title, work.selected, { actions.setWorkSelected(work.key, it) }, expanded, { actions.toggleExpanded(work.key) },
        summary = {
            ItemTitle(title, work.selected)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Chip(view?.quest)
                Detail(listOfNotNull(view?.whenLabel, view?.duration).joinToString(" · "))
            }
            if (work.task is DraftWorkTask.New) Detail(stringResource(R.string.capture_new_task))
            if (work.uncertain) Unsure(R.string.capture_unsure_task)
            ItemError(view?.error)
        },
        fields = { WorkFields(work, view, options, actions) },
    )
}

private fun workTitle(task: DraftWorkTask) = when (task) {
    is DraftWorkTask.Existing -> task.title
    is DraftWorkTask.New -> task.title
}

@Composable
private fun WorkFields(work: DraftWork, view: DraftWorkView?, options: List<DraftQuestOption>, actions: ProposalActions) {
    val key = work.key
    val task = work.task
    if (task is DraftWorkTask.New) {
        TitleField(stringResource(R.string.task_title), task.title, "capture-work-title") { actions.editWorkTitle(key, it) }
        Caps(stringResource(R.string.task_quest))
        QuestChoice(options, task.quest, "capture-work-quest") { actions.setWorkQuest(key, it) }
    }
    DateField(stringResource(R.string.form_date), work.day.label, work.day.iso, { actions.setWorkDay(key, it) }, "capture-work-date")
    if (view != null) {
        Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium), verticalAlignment = Alignment.Bottom) {
            TimeField(stringResource(R.string.form_start), view.startLabel, work.startMinutes, { actions.setWorkStart(key, it) }, "capture-work-start", Modifier.weight(1f))
            TimeField(stringResource(R.string.form_end), view.endLabel, work.endMinutes, { actions.setWorkEnd(key, it) }, "capture-work-end", Modifier.weight(1f))
        }
    }
}

@Composable
private fun TitleField(label: String, value: String, tag: String, onChange: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().testTag(tag),
    )
}

/** The quest picker over the core's options (active quests, then the draft's selected new ones). */
@Composable
private fun QuestChoice(options: List<DraftQuestOption>, selected: DraftQuestRef?, tag: String, onPick: (DraftQuestRef?) -> Unit) {
    val refs = options.map { QuestRef(pickerId(it.quest), it.chip.name, it.chip.colorHex) }
    val chosen = selected?.let { ref -> refs.firstOrNull { it.id == pickerId(ref) } }
    QuestPickerChip(chosen, refs, { picked -> onPick(options.firstOrNull { pickerId(it.quest) == picked?.id }?.quest) }, stringResource(R.string.task_quest), tag)
}

/** Existing and new quests apart in the picker. */
private fun pickerId(ref: DraftQuestRef) = when (ref) {
    is DraftQuestRef.Existing -> "existing:${ref.id}"
    is DraftQuestRef.New -> "new:${ref.key}"
}
