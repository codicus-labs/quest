package app.quest.android.ui.account

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.SyncPill
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import app.quest.android.ui.theme.StatusColors
import uniffi.quest_mobile.SyncState

/** The widest forms grow on tablets. */
val FormMaxWidth = 560.dp

class AccountActions(
    val signIn: (serverUrl: String) -> Unit = {},
    val signInAgain: () -> Unit = {},
    val askSignOut: () -> Unit = {},
    val serverUrlEdited: () -> Unit = {},
    val openSettings: () -> Unit = {},
    val openArchive: () -> Unit = {},
    val dialogs: AccountDialogActions = AccountDialogActions(),
)

/** The guest's sign-in, or the signed-in account with sign out (and sign in again once it expired); the way to Settings. */
@Composable
fun AccountScreen(state: AccountUiState, actions: AccountActions) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        Column(
            modifier = Modifier
                .widthIn(max = FormMaxWidth)
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(QuestSpacing.screen)
                .testTag("account"),
            verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
        ) {
            StatusCard(state)
            state.message?.let { Text(it, style = QuestType.body, color = StatusColors.failed, modifier = Modifier.testTag("account-message")) }
            state.busy?.let { Busy(it) }
            val working = state.busy != null && state.busy != AccountBusy.WaitingForBrowser
            when {
                state.account == null -> SignInForm(state.serverUrlProblem, enabled = !working, actions)
                state.sync == SyncState.SessionExpired -> Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.listGap)) {
                    Button(onClick = actions.signInAgain, enabled = !working, modifier = Modifier.testTag("sign-in-again")) {
                        Text(stringResource(R.string.account_sign_in_again))
                    }
                    OutlinedButton(onClick = actions.askSignOut, enabled = !working, modifier = Modifier.testTag("sign-out")) {
                        Text(stringResource(R.string.account_sign_out))
                    }
                }
                else -> OutlinedButton(onClick = actions.askSignOut, enabled = !working, modifier = Modifier.testTag("sign-out")) {
                    Text(stringResource(R.string.account_sign_out))
                }
            }
            NavRow(Icons.Filled.Settings, stringResource(R.string.settings), "open-settings", actions.openSettings, Modifier.padding(top = QuestSpacing.medium))
            NavRow(QuestIcons.Archive, stringResource(R.string.archive), "open-archive", actions.openArchive)
        }
    }
    state.dialog?.let { AccountDialogs(it, actions.dialogs) }
}

@Composable
private fun StatusCard(state: AccountUiState) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    Column(
        Modifier.fillMaxWidth().background(colors.card, shape).border(QuestSpacing.hairline, colors.border, shape).padding(QuestSpacing.large),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.tight),
    ) {
        val account = state.account
        if (account == null) {
            Text(stringResource(R.string.account_guest_title), style = QuestType.bodyMedium, color = colors.text)
            Text(stringResource(R.string.account_guest), style = QuestType.body, color = colors.textMuted, modifier = Modifier.testTag("account-info"))
            return@Column
        }
        SyncPill(state.sync)
        Text(
            stringResource(R.string.account_signed_in, account.displayName, account.serverUrl),
            style = QuestType.body,
            color = colors.textMuted,
            modifier = Modifier.testTag("account-info"),
        )
        when (val sync = state.sync) {
            is SyncState.Failed -> Text(sync.message, style = QuestType.small, color = StatusColors.failed, modifier = Modifier.testTag("sync-error"))
            SyncState.SessionExpired -> Text(
                stringResource(R.string.account_session_expired),
                style = QuestType.small,
                color = StatusColors.failed,
                modifier = Modifier.testTag("sync-error"),
            )
            else -> Unit
        }
    }
}

@Composable
private fun Busy(busy: AccountBusy) {
    val label = when (busy) {
        AccountBusy.WaitingForBrowser -> R.string.account_waiting_for_browser
        AccountBusy.SigningIn -> R.string.account_signing_in
        AccountBusy.Importing -> R.string.account_importing
        AccountBusy.SigningOut -> R.string.account_signing_out
    }
    Row(
        Modifier.semantics { liveRegion = LiveRegionMode.Polite }.testTag("account-busy"),
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
        Text(stringResource(label), style = QuestType.body, color = QuestTheme.colors.textMuted)
    }
}

@Composable
private fun SignInForm(serverUrlProblem: String?, enabled: Boolean, actions: AccountActions) {
    var server by rememberSaveable { mutableStateOf("") }
    OutlinedTextField(
        value = server,
        onValueChange = {
            server = it
            actions.serverUrlEdited()
        },
        label = { Text(stringResource(R.string.account_server_url)) },
        placeholder = { Text("https://quest.example.com") },
        singleLine = true,
        enabled = enabled,
        isError = serverUrlProblem != null,
        supportingText = serverUrlProblem?.let { { Text(it, modifier = Modifier.testTag("server-url-problem")) } },
        modifier = Modifier.fillMaxWidth().testTag("server-url"),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
        keyboardActions = KeyboardActions(onGo = { actions.signIn(server) }),
    )
    Button(onClick = { actions.signIn(server) }, enabled = enabled, modifier = Modifier.testTag("sign-in")) {
        Text(stringResource(R.string.account_sign_in))
    }
    Text(stringResource(R.string.account_sign_in_note), style = QuestType.small, color = QuestTheme.colors.textFaint)
}

@Composable
private fun NavRow(icon: ImageVector, label: String, tag: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    Row(
        modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = QuestSpacing.medium)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        Icon(icon, contentDescription = null, tint = colors.textMuted)
        Text(label, style = QuestType.bodyMedium, color = colors.text, modifier = Modifier.weight(1f))
        Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null, tint = colors.textFaint)
    }
}
