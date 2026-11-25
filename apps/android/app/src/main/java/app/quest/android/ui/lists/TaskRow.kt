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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.CompleteToggle
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.questColor
import app.quest.android.ui.theme.QuestColors
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.ListRow
import uniffi.quest_mobile.MarkTone
import uniffi.quest_mobile.RowMark

/** Text on the accent fill (the Stop pill): dark in both appearances, as on the desktop. */
val OnAccentFill = Color(0xFF1A1A1A)

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
        CompleteToggle(row.done, questTint ?: colors.accentFill, onToggle = { actions.toggle(row.id) })
        Column(Modifier.weight(1f).padding(vertical = QuestSpacing.small), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                row.title,
                style = QuestType.task,
                color = if (row.done) colors.textMuted else colors.text,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (row.marks.isNotEmpty()) Marks(row.marks, colors)
        }
        if (questTint != null) Dot(questTint, 8.dp, Modifier.padding(horizontal = QuestSpacing.small))
        when {
            row.timerRunning -> StopPill(row.timer.orEmpty(), row.title, actions.stopTimer)
            row.canStartTimer -> IconButton(onClick = { actions.startTimer(row.id) }, modifier = Modifier.testTag("timer-start:${row.title}")) {
                Icon(Icons.Filled.PlayArrow, contentDescription = stringResource(R.string.timer_start), tint = colors.textMuted)
            }
        }
    }
}

/** "overdue · Mon 23" in red, "due today" in the accent, the rest muted, one line. */
@Composable
private fun Marks(marks: List<RowMark>, colors: QuestColors) {
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
    Text(text, style = QuestType.small, maxLines = 1, overflow = TextOverflow.Ellipsis)
}

/** The running timer on its row: the time on the accent fill; tapping stops it. */
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
