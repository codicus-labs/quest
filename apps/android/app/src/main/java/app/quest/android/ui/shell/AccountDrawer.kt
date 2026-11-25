package app.quest.android.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.SyncPill
import app.quest.android.ui.components.OnAccentFill
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.Account
import uniffi.quest_mobile.SyncState
import uniffi.quest_mobile.WorkspacesData

/** What the drawer shows: the workspaces, the account (null: guest) and its sync. */
class DrawerData(val workspaces: WorkspacesData, val account: Account?, val sync: SyncState)

class DrawerActions(
    val close: () -> Unit,
    val switchTo: (String) -> Unit,
    /** New, rename and delete: the Manage workspaces sheet. */
    val manageWorkspaces: () -> Unit,
    /** Sign in (guest) or the account's page. */
    val openAccount: () -> Unit,
    val signOut: () -> Unit,
    val openSettings: () -> Unit,
    val openArchive: () -> Unit,
) {
    companion object {
        val None = DrawerActions({}, {}, {}, {}, {}, {}, {})
    }
}

/** The Account tab: a bottom sheet with the workspaces, the account and the settings. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AccountDrawer(data: DrawerData, actions: DrawerActions) {
    ModalBottomSheet(
        onDismissRequest = actions.close,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = QuestTheme.colors.card,
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag("account-drawer"),
    ) {
        AccountDrawerContent(data, actions)
    }
}

/** The drawer's inside, without the sheet (screenshots draw it on its own). */
@Composable
fun AccountDrawerContent(data: DrawerData, actions: DrawerActions) {
    val colors = QuestTheme.colors
    Column(
        Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = QuestSpacing.screen).navigationBarsPadding(),
    ) {
        Row(
            Modifier.padding(horizontal = QuestSpacing.screen, vertical = QuestSpacing.small),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
        ) {
            Avatar(data.workspaces.active.name, 40.dp)
            Column(Modifier.weight(1f)) {
                Text(data.workspaces.active.name, style = QuestType.stat, color = colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.semantics { heading() })
                Text(data.account?.displayName ?: stringResource(R.string.account_guest_title), style = QuestType.small, color = colors.textMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        Section(stringResource(R.string.drawer_workspaces))
        data.workspaces.live.forEach { workspace ->
            DrawerRow(
                label = workspace.name,
                tag = "drawer-workspace:${workspace.name}",
                onClick = { if (!workspace.current) actions.switchTo(workspace.id) },
                modifier = Modifier.semantics { selected = workspace.current },
                leading = { Avatar(workspace.name, 24.dp) },
                trailing = if (workspace.current) {
                    { Icon(Icons.Filled.Check, contentDescription = stringResource(R.string.workspace_current), tint = colors.text) }
                } else {
                    null
                },
            )
        }
        DrawerRow(stringResource(R.string.workspaces_new), "drawer-new-workspace", actions.manageWorkspaces, leading = { RowIcon(Icons.Filled.Add) })
        DrawerRow(stringResource(R.string.workspaces_manage_entry), "drawer-manage-workspaces", actions.manageWorkspaces, leading = { RowIcon(QuestIcons.Layers) })
        Section(stringResource(R.string.account))
        val account = data.account
        Column(Modifier.padding(horizontal = QuestSpacing.screen, vertical = QuestSpacing.tight), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            if (account != null) {
                Text(account.displayName, style = QuestType.body, color = colors.text, modifier = Modifier.testTag("drawer-account-name"))
                Text(account.serverUrl, style = QuestType.small, color = colors.textMuted)
            }
            SyncPill(data.sync)
        }
        if (account == null) {
            DrawerRow(stringResource(R.string.account_sign_in), "drawer-sign-in", actions.openAccount)
        } else {
            DrawerRow(stringResource(R.string.drawer_account_details), "open-account", actions.openAccount)
            DrawerRow(stringResource(R.string.account_sign_out), "drawer-sign-out", actions.signOut)
        }
        HorizontalDivider(Modifier.padding(vertical = QuestSpacing.small), color = colors.border)
        DrawerRow(stringResource(R.string.settings), "open-settings", actions.openSettings, leading = { RowIcon(Icons.Filled.Settings) })
        DrawerRow(stringResource(R.string.archive), "open-archive", actions.openArchive, leading = { RowIcon(QuestIcons.Archive) })
    }
}

/** A workspace's first letter on the accent fill: the Account tab's icon and the drawer's rows. */
@Composable
fun Avatar(name: String, size: Dp, modifier: Modifier = Modifier) {
    Box(modifier.size(size).background(QuestTheme.colors.accentFill, CircleShape), contentAlignment = Alignment.Center) {
        Text(
            name.trim().take(1).uppercase(),
            style = if (size >= 32.dp) QuestType.stat else QuestType.smallMedium,
            color = OnAccentFill,
        )
    }
}

@Composable
private fun Section(title: String) {
    Text(
        title,
        style = QuestType.caps,
        color = QuestTheme.colors.textMuted,
        modifier = Modifier.padding(start = QuestSpacing.screen, top = QuestSpacing.large, bottom = QuestSpacing.tight).semantics { heading() },
    )
}

@Composable
private fun RowIcon(icon: ImageVector) {
    Icon(icon, contentDescription = null, tint = QuestTheme.colors.textMuted, modifier = Modifier.size(24.dp))
}

@Composable
private fun DrawerRow(
    label: String,
    tag: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = QuestSpacing.touchTarget)
            .testTag(tag)
            .clickable(onClick = onClick)
            .padding(horizontal = QuestSpacing.screen),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        leading?.invoke()
        Text(label, style = QuestType.body, color = QuestTheme.colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        trailing?.invoke()
    }
}
