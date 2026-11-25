package app.quest.android.ui.lists

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.CompleteToggle
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.OnAccentFill
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.TaskTimerIcon
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestColors
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.ListRow
import uniffi.quest_mobile.MarkTone
import uniffi.quest_mobile.RowMark

/** What a row can do. */
class RowActions(
    val toggle: (String) -> Unit,
    val open: (String) -> Unit,
    val startTimer: (String) -> Unit,
    val stopTimer: () -> Unit,
)

/** One task: check, title, its date marks below, the quest's dot, and Play or the running timer's Stop. */
@Composable
fun TaskRow(row: ListRow, actions: RowActions, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val questTint = row.quest?.let { questColor(it.colorHex) }
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .clickable { actions.open(row.id) }
            .padding(end = QuestSpacing.small)
            .testTag("row:${row.title}"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CompleteToggle(row.done, onToggle = { actions.toggle(row.id) })
        Column(Modifier.weight(1f).padding(vertical = QuestSpacing.small), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                row.title,
                style = QuestType.task,
                color = if (row.done) colors.textMuted else colors.text,
                textDecoration = if (row.done) TextDecoration.LineThrough else null,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            val progress = row.checklistProgress
            if (progress == null) {
                if (row.marks.isNotEmpty()) Marks(row.marks, colors)
            } else {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (row.marks.isNotEmpty()) Marks(row.marks, colors, Modifier.weight(1f, fill = false))
                    Text("${progress.done}/${progress.total}", style = QuestType.small, color = colors.textMuted)
                }
            }
        }
        if (questTint != null) Dot(questTint, 8.dp, Modifier.padding(horizontal = QuestSpacing.small))
        if (row.timerRunning || row.canStartTimer) {
            val action = if (row.timerRunning) "stop" else "start"
            IconButton(
                onClick = { if (row.timerRunning) actions.stopTimer() else actions.startTimer(row.id) },
                modifier = Modifier.testTag("timer-$action:${row.title}"),
            ) { TaskTimerIcon(row.timerRunning) }
        }
    }
}

/** "overdue · Mon 23" in red, "due today" in the accent, the rest muted, one line. */
@Composable
private fun Marks(marks: List<RowMark>, colors: QuestColors, modifier: Modifier = Modifier) {
    val text = buildAnnotatedString {
        marks.forEachIndexed { i, mark ->
            if (i > 0) withStyle(SpanStyle(color = colors.textFaint)) { append("  ·  ") }
            val color = when (mark.tone) {
                MarkTone.OVERDUE -> colors.error
                MarkTone.ACCENT -> colors.accent
                MarkTone.MUTED -> colors.textMuted
            }
            withStyle(SpanStyle(color = color)) { append(mark.text) }
        }
    }
    Text(text, modifier = modifier, style = QuestType.small, maxLines = 1, overflow = TextOverflow.Ellipsis)
}

/** The mini-player's timer and stop control. */
@Composable
fun StopPill(timer: String, title: String, onStop: () -> Unit, modifier: Modifier = Modifier) {
    val label = stringResource(R.string.timer_stop_label, timer)
    val shape = RoundedCornerShape(50)
    Row(
        modifier
            .minimumInteractiveComponentSize()
            .heightIn(min = 32.dp)
            .background(QuestTheme.colors.accentFill, shape)
            .clickable(onClick = onStop)
            .semantics(mergeDescendants = true) {
                role = Role.Button
                contentDescription = label
            }
            .padding(horizontal = 10.dp)
            .testTag("timer-stop:$title"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Icon(QuestIcons.Stop, contentDescription = null, tint = OnAccentFill, modifier = Modifier.size(14.dp))
        Text(timer, style = QuestType.monoTime, color = OnAccentFill)
    }
}
