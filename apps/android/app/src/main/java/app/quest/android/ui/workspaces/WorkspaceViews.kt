package app.quest.android.ui.workspaces

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.TimerElsewhere
import uniffi.quest_mobile.WorkspacesData

/**
 * "● 36:00 in Default": the focus timer runs in another workspace than the shown one (the timer is
 * one per person). Tapping it shows that workspace on Focus.
 */
@Composable
fun TimerElsewhereNote(timer: TimerElsewhere, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val label = stringResource(R.string.workspace_timer_label, timer.workspaceName, timer.taskTitle)
    Row(
        modifier
            .heightIn(min = QuestSpacing.touchTarget)
            .clickable(role = Role.Button, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = label }
            .testTag("timer-elsewhere"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            Modifier
                .background(colors.card, RoundedCornerShape(16.dp))
                .border(BorderStroke(QuestSpacing.hairline, colors.borderStrong), RoundedCornerShape(16.dp))
                .padding(horizontal = 12.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small),
        ) {
            Spacer(Modifier.size(8.dp).background(colors.accent, CircleShape))
            Text(timer.time, style = QuestType.monoTime, color = colors.text)
            Text(
                stringResource(R.string.workspace_timer_in, timer.workspaceName),
                style = QuestType.small,
                color = colors.textMuted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.widthIn(max = 160.dp),
            )
        }
    }
}

/** A row of the switcher's width under the top bar, for [TimerElsewhereNote]. */
@Composable
fun TimerElsewhereBar(data: WorkspacesData?, onClick: () -> Unit) {
    val timer = data?.timerElsewhere ?: return
    Column(Modifier.fillMaxWidth().padding(horizontal = QuestSpacing.large)) {
        TimerElsewhereNote(timer, onClick)
    }
}

/** A plain outlined button at touch size, as the sheet's rows use it. */
@Composable
internal fun SheetButton(label: String, tag: String, enabled: Boolean = true, description: String = label, danger: Boolean = false, onClick: () -> Unit) {
    val colors = QuestTheme.colors
    OutlinedButton(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier.heightIn(min = QuestSpacing.touchTarget).semantics { contentDescription = description }.testTag(tag),
    ) {
        Text(label, style = QuestType.bodyMedium, color = if (danger) colors.error else if (enabled) colors.text else colors.textFaint)
    }
}
