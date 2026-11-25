package app.quest.android.ui.plan

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.TaskCardActions
import app.quest.android.ui.home.SwipeTaskCard
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.PlanDay
import uniffi.quest_mobile.QuestChip as QuestRef

/** Everything the plan can do, wired to [PlanViewModel] (and the task sheet, Focus). */
class PlanActions(
    val toggleTask: (String) -> Unit,
    val openTask: (String) -> Unit,
    val startTimer: (String) -> Unit,
    val stopTimer: () -> Unit,
    val deleteTask: (String) -> Unit,
    val undo: () -> Unit,
    val dismissToast: () -> Unit,
    val editDraft: (day: String, text: String) -> Unit,
    val addTask: (day: String) -> Unit,
    val setAddPosition: (AddPosition) -> Unit,
    val setQuestFilter: (String?) -> Unit,
    val setShowOther: (Boolean) -> Unit,
    /** The screen shows this many days from this one (ISO). */
    val visibleRange: (firstDay: String, count: Int) -> Unit,
    /** Jump to a day (ISO; null: today). */
    val jumpTo: (String?) -> Unit,
    val scrolled: (ScrollRequest) -> Unit,
    val moveTask: (taskId: String, day: String?, beforeTaskId: String?) -> Unit,
    val nudgeTask: (taskId: String, up: Boolean) -> Unit,
    val shiftTask: (taskId: String, later: Boolean) -> Unit,
    val openMoveTo: (taskId: String, title: String) -> Unit,
    val moveTo: (day: String?) -> Unit,
    val closeMoveTo: () -> Unit,
    /** The core's edge rule (`plan_edge_speed`): dp/s for a drag at pos in start..end (dp). */
    val edgeSpeed: (pos: Float, start: Float, end: Float) -> Float,
    /** Gives a task without a quest one (the card's "Assign quest" icon). */
    val assignQuest: (taskId: String, questId: String) -> Unit = { _, _ -> },
) {
    companion object {
        /** Does nothing; for previews and screenshots. */
        val None = PlanActions(
            {}, {}, {}, {}, {}, {}, {}, { _, _ -> }, {}, {}, {}, {}, { _, _ -> }, {}, {},
            { _, _, _ -> }, { _, _ -> }, { _, _ -> }, { _, _ -> }, {}, {}, { _, _, _ -> 0f },
        )
    }
}

/** Compact list: the sticky day head ("Tuesday, November 24"); today in the accent colour. */
@Composable
fun DayHead(day: PlanDay, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    Column(modifier.fillMaxWidth().background(colors.bg).padding(top = QuestSpacing.medium)) {
        Text(
            day.label,
            style = QuestType.bodyMedium,
            color = if (day.isToday) colors.accent else if (day.isPast) colors.textFaint else colors.text,
            modifier = Modifier.padding(bottom = QuestSpacing.tight).semantics { heading() }.testTag("plan-day:${day.dayIso}"),
        )
        HorizontalDivider(thickness = if (day.isToday) 2.dp else QuestSpacing.hairline, color = if (day.isToday) colors.accent else colors.border)
    }
}

/** Column board: the column head ("Tue 24", desktop), underlined; today's in the accent colour. */
@Composable
fun ColumnHead(day: PlanDay, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    Column(modifier.fillMaxWidth()) {
        Row(
            Modifier.padding(bottom = QuestSpacing.small).semantics(mergeDescendants = true) { heading() }.testTag("plan-day:${day.dayIso}"),
            horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small),
            verticalAlignment = Alignment.Bottom,
        ) {
            Text(day.weekday, style = QuestType.stat, color = if (day.isToday) colors.text else colors.textMuted)
            Text(day.dayNumber, style = QuestType.bodyMedium, color = if (day.isToday) colors.accent else colors.textFaint, modifier = Modifier.padding(bottom = 2.dp))
        }
        HorizontalDivider(thickness = 2.dp, color = if (day.isToday) colors.accent else colors.border)
    }
}

/** "OTHER" above a day's other tasks (desktop column divider). */
@Composable
fun OtherDivider() {
    val colors = QuestTheme.colors
    Row(Modifier.fillMaxWidth().padding(top = QuestSpacing.tight), horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small), verticalAlignment = Alignment.CenterVertically) {
        Text(stringResource(R.string.plan_other).uppercase(), style = QuestType.caps, color = colors.textMuted)
        HorizontalDivider(Modifier.weight(1f), color = colors.border)
    }
}

/**
 * The day's add field (desktop "Add a task…"): one line, where new tasks go (top/bottom, tap to
 * switch), and an add button once there is text. Done on the keyboard adds too.
 */
@Composable
fun PlanAddField(day: String, text: String, position: AddPosition, actions: PlanActions, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.fieldRadius)
    val placeholder = stringResource(R.string.plan_add_task)
    // The field owns what is typed and reports it to the ViewModel (saved state). Mirroring the
    // ViewModel back would overwrite fast typing with a stale draft, so only its clearing after
    // an add comes back.
    val field = rememberTextFieldState(text)
    LaunchedEffect(field) { snapshotFlow { field.text.toString() }.collect { actions.editDraft(day, it) } }
    LaunchedEffect(text) { if (text.isEmpty()) field.clearText() }
    BasicTextField(
        state = field,
        lineLimits = TextFieldLineLimits.SingleLine,
        textStyle = QuestType.body.copy(color = colors.text),
        cursorBrush = SolidColor(colors.accent),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        onKeyboardAction = { actions.addTask(day) },
        modifier = modifier.fillMaxWidth().semantics { contentDescription = placeholder }.testTag("plan-add:$day"),
        decorator = { inner ->
            Row(
                Modifier.heightIn(min = QuestSpacing.touchTarget).border(QuestSpacing.hairline, colors.border, shape).padding(start = QuestSpacing.medium),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(Modifier.weight(1f)) {
                    if (field.text.isEmpty()) Text(placeholder, style = QuestType.body, color = colors.textFaint)
                    inner()
                }
                PositionToggle(day, position, actions.setAddPosition)
                if (field.text.isNotBlank()) {
                    IconButton(onClick = { actions.addTask(day) }, modifier = Modifier.size(QuestSpacing.cardControl).testTag("plan-add-send:$day")) {
                        Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.home_add_task), tint = colors.accent)
                    }
                }
            }
        },
    )
}

@Composable
private fun PositionToggle(day: String, position: AddPosition, onChange: (AddPosition) -> Unit) {
    val top = position == AddPosition.TOP
    val state = stringResource(if (top) R.string.home_add_top else R.string.home_add_bottom)
    val label = stringResource(R.string.home_add_position)
    IconButton(
        onClick = { onChange(if (top) AddPosition.BOTTOM else AddPosition.TOP) },
        modifier = Modifier
            .size(QuestSpacing.cardControl)
            .semantics {
                contentDescription = label
                stateDescription = state
                role = Role.Switch
            }
            .testTag("plan-add-position:$day"),
    ) {
        Icon(if (top) QuestIcons.ToTop else QuestIcons.ToBottom, contentDescription = null, tint = QuestTheme.colors.textMuted, modifier = Modifier.size(18.dp))
    }
}

/**
 * A plan card: Home's swipeable [SwipeTaskCard] (dimmed on past days, faded while it is the one
 * being dragged), the drop line when a drag would land here, and the moves as accessibility actions.
 */
@Composable
fun PlanCard(row: PlanRow.Card, quests: List<QuestRef>, drag: PlanDragState, actions: PlanActions, modifier: Modifier = Modifier) {
    val task = row.task
    val dragged = drag.isDragged(task.id)
    val line = drag.lineFor(row.key)
    val lineColor = QuestTheme.colors.accent
    val moveLabels = MoveLabels.current()
    Box(
        modifier
            // Before alpha: its layer clips, and the line sits in the gap outside the card.
            .drawWithContent {
                drawContent()
                if (line == null) return@drawWithContent
                // In the middle of the gap between cards (desktop LIST_GAP).
                val gap = QuestSpacing.listGap.toPx()
                val thickness = 3.dp.toPx()
                val y = if (line.above) -gap / 2 - thickness / 2 else size.height + gap / 2 - thickness / 2
                drawRect(lineColor, Offset(0f, y), Size(size.width, thickness))
            }
            .alpha(if (dragged) 0.3f else if (row.past) 0.8f else 1f),
    ) {
        SwipeTaskCard(
            task = task,
            actions = TaskCardActions(
                onToggle = { actions.toggleTask(task.id) },
                onOpen = { actions.openTask(task.id) },
                onTimer = { if (task.timerRunning) actions.stopTimer() else actions.startTimer(task.id) },
                onAssignQuest = { questId -> actions.assignQuest(task.id, questId) },
                questOptions = quests,
            ),
            onDelete = { actions.deleteTask(task.id) },
            swipeEnabled = !drag.active,
            extraActions = listOf(
                CustomAccessibilityAction(moveLabels.moveTo) { actions.openMoveTo(task.id, task.title); true },
                CustomAccessibilityAction(moveLabels.up) { actions.nudgeTask(task.id, true); true },
                CustomAccessibilityAction(moveLabels.down) { actions.nudgeTask(task.id, false); true },
                CustomAccessibilityAction(moveLabels.previousDay) { actions.shiftTask(task.id, false); true },
                CustomAccessibilityAction(moveLabels.nextDay) { actions.shiftTask(task.id, true); true },
            ),
        )
    }
}

/** The move actions' labels, read once per card. */
private class MoveLabels(val moveTo: String, val up: String, val down: String, val previousDay: String, val nextDay: String) {
    companion object {
        @Composable
        fun current() = MoveLabels(
            stringResource(R.string.plan_move_to),
            stringResource(R.string.plan_move_up),
            stringResource(R.string.plan_move_down),
            stringResource(R.string.plan_move_previous_day),
            stringResource(R.string.plan_move_next_day),
        )
    }
}

/** One plan row (not the compact list's sticky head). */
@Composable
fun PlanRowContent(row: PlanRow, state: PlanUiState.Ready, drag: PlanDragState, actions: PlanActions, modifier: Modifier = Modifier) {
    when (row) {
        is PlanRow.Head -> DayHead(row.plan, modifier)
        is PlanRow.Add -> PlanAddField(row.day, state.inputs.draft(row.day), state.prefs.addPosition, actions, modifier)
        is PlanRow.OtherDivider -> Box(modifier) { OtherDivider() }
        is PlanRow.Card -> PlanCard(row, state.plan.quests, drag, actions, modifier)
    }
}

/** An empty spot at the end of a list, so the last card clears the undo snackbar. */
@Composable
fun ListEnd() {
    Box(Modifier.fillMaxWidth().height(88.dp))
}

/** A full-width tappable row in a sheet. */
@Composable
fun SheetRow(text: String, tag: String, onClick: () -> Unit) {
    Text(
        text,
        style = QuestType.task,
        color = QuestTheme.colors.text,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = QuestSpacing.touchTarget)
            .clickable(onClick = onClick)
            .padding(vertical = QuestSpacing.medium)
            .testTag(tag),
    )
}
