package app.quest.android.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.SectionHeader
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import app.quest.android.ui.theme.StatusColors
import uniffi.quest_mobile.AiConnectionRow

/** The "Connected AI clients" section: hidden while signed out, else loading, the list, or why it is missing. */
sealed interface AiConnectionsState {
    /** Signed out: no section. */
    data object Hidden : AiConnectionsState

    data object Loading : AiConnectionsState

    data class Loaded(
        val rows: List<AiConnectionRow>,
        /** The client whose Revoke waits for confirmation. */
        val confirming: AiConnectionRow? = null,
        /** The id being revoked right now. */
        val revoking: String? = null,
        /** Why the last revoke did not work. */
        val error: String? = null,
    ) : AiConnectionsState

    /** [message]: the core's; [offline]: the server was not reachable. */
    data class Failed(val message: String, val offline: Boolean) : AiConnectionsState
}

class AiConnectionsActions(
    val reload: () -> Unit = {},
    val askRevoke: (AiConnectionRow) -> Unit = {},
    val cancelRevoke: () -> Unit = {},
    val revoke: () -> Unit = {},
)

@Composable
fun AiConnectionsSection(state: AiConnectionsState, actions: AiConnectionsActions) {
    if (state == AiConnectionsState.Hidden) return
    val colors = QuestTheme.colors
    SectionHeader(stringResource(R.string.connections_title), Modifier.padding(top = QuestSpacing.medium))
    Text(stringResource(R.string.connections_note), style = QuestType.small, color = colors.textMuted)
    when (state) {
        AiConnectionsState.Hidden -> {}
        AiConnectionsState.Loading -> Row(
            Modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget).testTag("connections-loading"),
            horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
            Text(stringResource(R.string.connections_loading), style = QuestType.body, color = colors.textMuted)
        }
        is AiConnectionsState.Failed -> Problem(
            if (state.offline) stringResource(R.string.connections_offline) else state.message,
            actions.reload,
        )
        is AiConnectionsState.Loaded -> {
            if (state.rows.isEmpty()) {
                Text(stringResource(R.string.connections_empty), style = QuestType.body, color = colors.textMuted, modifier = Modifier.testTag("connections-empty"))
            }
            state.rows.forEach { row -> ConnectionRow(row, revoking = state.revoking == row.id, enabled = state.revoking == null, onRevoke = { actions.askRevoke(row) }) }
            state.error?.let { Text(it, style = QuestType.body, color = StatusColors.failed, modifier = Modifier.testTag("connections-error")) }
            state.confirming?.let { RevokeDialog(it, actions) }
        }
    }
}

@Composable
private fun ConnectionRow(row: AiConnectionRow, revoking: Boolean, enabled: Boolean, onRevoke: () -> Unit) {
    val colors = QuestTheme.colors
    Row(Modifier.fillMaxWidth().heightIn(min = QuestSpacing.touchTarget).testTag("connection-${row.id}"), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(row.name, style = QuestType.body, color = colors.text)
            Text("${row.host} · ${row.scope}", style = QuestType.small, color = colors.textMuted, modifier = Modifier.testTag("connection-scope"))
            Text(row.detail, style = QuestType.small, color = colors.textFaint)
        }
        val revokingLabel = stringResource(R.string.connections_revoking)
        val revokeLabel = stringResource(R.string.connections_revoke_named, row.name)
        TextButton(
            onClick = onRevoke,
            enabled = enabled,
            modifier = Modifier.testTag("connection-revoke").semantics {
                contentDescription = revokeLabel
                if (revoking) stateDescription = revokingLabel
            },
        ) {
            when {
                revoking -> CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                // Disabled while another revoke runs: the button's own disabled colour, not red.
                enabled -> Text(stringResource(R.string.connections_revoke), color = StatusColors.failed)
                else -> Text(stringResource(R.string.connections_revoke))
            }
        }
    }
}

@Composable
private fun Problem(message: String, retry: () -> Unit) {
    Column(Modifier.fillMaxWidth().testTag("connections-failed")) {
        Text(message, style = QuestType.body, color = QuestTheme.colors.textMuted)
        TextButton(onClick = retry, modifier = Modifier.testTag("connections-retry")) { Text(stringResource(R.string.connections_retry)) }
    }
}

@Composable
private fun RevokeDialog(row: AiConnectionRow, actions: AiConnectionsActions) {
    AlertDialog(
        onDismissRequest = actions.cancelRevoke,
        title = { Text(stringResource(R.string.connections_revoke_title, row.name)) },
        text = { Text(stringResource(R.string.connections_revoke_text)) },
        confirmButton = {
            TextButton(onClick = actions.revoke, modifier = Modifier.testTag("connection-revoke-confirm")) {
                Text(stringResource(R.string.connections_revoke), color = StatusColors.failed)
            }
        },
        dismissButton = { TextButton(onClick = actions.cancelRevoke) { Text(stringResource(R.string.cancel)) } },
    )
}
