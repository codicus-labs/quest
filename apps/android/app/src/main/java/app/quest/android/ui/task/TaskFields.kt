package app.quest.android.ui.task

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.OnAccentFill
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.CalendarMonth
import uniffi.quest_mobile.DateTarget
import uniffi.quest_mobile.QuestChip

enum class FieldPicker(val label: String, val target: DateTarget? = null) {
    Quest("Quest"), Plan("Plan", DateTarget.PLAN), Deadline("Deadline", DateTarget.DEADLINE),
}

@Composable
fun TaskFields(
    quest: QuestChip?,
    plan: String?,
    deadline: String?,
    onOpen: (FieldPicker) -> Unit,
    planMoves: UInt = 0u,
    deadlineMoves: UInt = 0u,
) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        FieldButton(FieldPicker.Quest, "${quest?.name ?: "No quest"} ▾", onOpen, quest)
        FieldButton(FieldPicker.Plan, dateLabel("▦", plan ?: "Plan", planMoves), onOpen)
        FieldButton(FieldPicker.Deadline, dateLabel("⚑", deadline ?: "Deadline", deadlineMoves), onOpen)
    }
}

private fun dateLabel(icon: String, label: String, moves: UInt) =
    if (moves == 0u) "$icon $label" else "$icon $label ↷ $moves"

@Composable
private fun FieldButton(picker: FieldPicker, label: String, onOpen: (FieldPicker) -> Unit, quest: QuestChip? = null) {
    OutlinedButton(
        onClick = { onOpen(picker) },
        modifier = Modifier.testTag("field-${picker.name.lowercase()}").semantics { contentDescription = picker.label },
        contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 10.dp, vertical = 4.dp),
        border = androidx.compose.foundation.BorderStroke(1.dp, QuestTheme.colors.border),
    ) {
        quest?.let { Dot(questColor(it.colorHex), 8.dp, Modifier.padding(end = 5.dp)) }
        Text(label, color = QuestTheme.colors.text, style = QuestType.small)
    }
}

@Composable
fun TaskQuestChoices(options: (String) -> List<QuestChip>, onPick: (QuestChip?) -> Unit) {
    var query by rememberSaveable { mutableStateOf("") }
    OutlinedTextField(
        value = query,
        onValueChange = { query = it },
        placeholder = { Text("Search quests") },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().testTag("task-quest-search"),
    )
    Column(Modifier.heightIn(max = 300.dp).verticalScroll(rememberScrollState())) {
        options(query).forEach { quest ->
            TextButton(onClick = { onPick(quest) }) {
                Dot(questColor(quest.colorHex), 8.dp, Modifier.padding(end = 8.dp))
                Text(quest.name, color = QuestTheme.colors.text)
            }
        }
        TextButton(onClick = { onPick(null) }) { Text("No quest", color = QuestTheme.colors.textMuted) }
    }
}

@Composable
fun TaskCalendarChoices(
    target: DateTarget,
    month: (String?) -> CalendarMonth?,
    shift: (String, Boolean) -> String?,
    onPick: (String?) -> Unit,
) {
    var cursor by rememberSaveable { mutableStateOf<String?>(null) }
    val model = month(cursor) ?: return
    val move = { forward: Boolean -> shift(model.cursor.iso, forward)?.let { cursor = it }; Unit }
    Column {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            model.quickChoices.forEach { choice ->
                TextButton(onClick = { onPick(choice.date?.iso) }) {
                    Text(choice.label, color = QuestTheme.colors.text, style = QuestType.small)
                }
            }
        }
        Column(Modifier.pointerInput(model.cursor.iso) {
            var drag = 0f
            detectHorizontalDragGestures(
                onDragStart = { drag = 0f },
                onDragEnd = { if (kotlin.math.abs(drag) > 40.dp.toPx()) move(drag < 0) },
            ) { change, amount -> change.consume(); drag += amount }
        }) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                IconButton(onClick = { move(false) }) {
                    Icon(Icons.AutoMirrored.Filled.KeyboardArrowLeft, "Previous month", tint = QuestTheme.colors.text)
                }
                Text(model.label, color = QuestTheme.colors.text, style = QuestType.nav)
                IconButton(onClick = { move(true) }) {
                    Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, "Next month", tint = QuestTheme.colors.text)
                }
            }
            Row(Modifier.fillMaxWidth()) {
                listOf("Mo", "Tu", "We", "Th", "Fr", "Sa", "Su").forEach { day ->
                    Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                        Text(day, color = QuestTheme.colors.textMuted, style = QuestType.small)
                    }
                }
            }
            model.cells.chunked(7).forEach { week ->
                Row(Modifier.fillMaxWidth()) {
                    week.forEach { cell ->
                        val colors = QuestTheme.colors
                        Box(
                            Modifier.weight(1f).heightIn(min = 40.dp)
                                .then(if (cell.selected) Modifier.background(colors.accentFill, CircleShape) else Modifier)
                                .clickable(enabled = cell.enabled, role = Role.Button) { onPick(cell.day.iso) }
                                .semantics {
                                    selected = cell.selected
                                    contentDescription = cell.day.label + if (cell.marker != null) {
                                        if (target == DateTarget.PLAN) ", deadline" else ", plan"
                                    } else ""
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            val tint = when {
                                cell.selected -> OnAccentFill
                                !cell.enabled || !cell.inMonth -> colors.textMuted
                                cell.today -> colors.accent
                                else -> colors.text
                            }
                            val marker = when (cell.marker) {
                                DateTarget.DEADLINE -> "◆"
                                DateTarget.PLAN -> "●"
                                null -> ""
                            }
                            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                                Text(cell.day.iso.takeLast(2).trimStart('0'), color = tint, style = QuestType.small)
                                if (marker.isNotEmpty()) Text(marker, color = tint, style = QuestType.caps)
                            }
                        }
                    }
                }
            }
        }
    }
}
