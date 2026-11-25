package app.quest.android.ui.workspaces

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.TimerElsewhere
import uniffi.quest_mobile.WorkspacesData

/** What the switcher and the timer note do; [QuestApp] wires them. */
class SwitcherActions(
    /** Shows a workspace (the app first closes what belongs to the shown one). */
    val switchTo: (String) -> Unit,
    val manage: () -> Unit,
    /** The timer note: show the timer's workspace on Focus. */
    val showTimer: () -> Unit,
) {
    companion object {
        val None = SwitcherActions({}, {}, {})
    }
}

/**
 * The shown workspace's name with a chevron, in the top bar (the desktop's switcher between the logo
 * and the tabs). Opens a menu: the live workspaces, the shown one checked, then "Manage workspaces…".
 */
@Composable
fun WorkspaceSwitcher(data: WorkspacesData, actions: SwitcherActions, modifier: Modifier = Modifier, startOpen: Boolean = false) {
    val colors = QuestTheme.colors
    var open by rememberSaveable { mutableStateOf(startOpen) }
    val label = stringResource(R.string.workspace_switch, data.active.name)
    Box(modifier) {
        Row(
            Modifier
                .heightIn(min = QuestSpacing.touchTarget)
                .clickable(role = Role.Button) { open = true }
                .semantics { contentDescription = label }
                .testTag("workspace-switcher"),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Row(
                Modifier
                    .border(BorderStroke(QuestSpacing.hairline, colors.borderStrong), RoundedCornerShape(7.dp))
                    .padding(horizontal = 10.dp, vertical = 7.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(7.dp),
            ) {
                Icon(QuestIcons.Layers, contentDescription = null, tint = colors.textMuted, modifier = Modifier.size(16.dp))
                Text(
                    data.active.name,
                    style = QuestType.nav,
                    color = colors.text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.widthIn(max = 160.dp).testTag("workspace-name"),
                )
                Icon(QuestIcons.ChevronDown, contentDescription = null, tint = colors.textMuted, modifier = Modifier.size(16.dp))
            }
        }
        DropdownMenu(
            expanded = open,
            onDismissRequest = { open = false },
            containerColor = colors.cardRaised,
            modifier = Modifier.widthIn(min = 220.dp).testTag("workspace-menu"),
        ) {
            data.live.forEachIndexed { index, workspace ->
                DropdownMenuItem(
                    text = { Text(workspace.name, style = QuestType.body, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    trailingIcon = if (workspace.current) {
                        { Icon(Icons.Filled.Check, contentDescription = stringResource(R.string.workspace_current), tint = colors.text) }
                    } else {
                        null
                    },
                    onClick = {
                        open = false
                        if (!workspace.current) actions.switchTo(workspace.id)
                    },
                    modifier = Modifier.semantics { selected = workspace.current }.testTag("workspace-menu-item$index"),
                )
            }
            HorizontalDivider(color = colors.border)
            DropdownMenuItem(
                text = { Text(stringResource(R.string.workspaces_manage_entry), style = QuestType.body) },
                onClick = {
                    open = false
                    actions.manage()
                },
                modifier = Modifier.testTag("workspace-manage"),
            )
        }
    }
}

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
