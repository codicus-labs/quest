package app.quest.android.ui.components

import androidx.compose.ui.semantics.heading
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import app.quest.android.R
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.StatusColors
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.SyncState

/** Quest's bottom sheet (used instead of dialogs): the Material sheet around [QuestSheetContent]. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QuestBottomSheet(title: String, onDismiss: () -> Unit, content: @Composable () -> Unit) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        // Full height only, app-wide: half open, the keyboard would cover the sheet's field and buttons.
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = QuestTheme.colors.card,
        // The sheet is its own window, so it needs its own opt-in for UI automation ids.
        modifier = Modifier.semantics { testTagsAsResourceId = true },
    ) {
        QuestSheetContent(title, content)
    }
}

/** The inside of a sheet: title, then the content, clear of the navigation bar. */
@Composable
fun QuestSheetContent(title: String, content: @Composable () -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = QuestSpacing.screen).padding(bottom = QuestSpacing.screen).navigationBarsPadding(),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
    ) {
        Text(title, style = QuestType.stat, color = QuestTheme.colors.text, modifier = Modifier.semantics { heading() })
        content()
    }
}

/** The app's snackbar host; snackbars with an action render as [UndoSnackbar]. */
@Composable
fun QuestSnackbarHost(state: SnackbarHostState, modifier: Modifier = Modifier) {
    SnackbarHost(state, modifier) { data -> QuestSnackbar(data.visuals.message, data.visuals.actionLabel, data::performAction) }
}

/** The core's undo toast as a snackbar: [message], an Undo action and (optionally) a close button. */
@Composable
fun UndoSnackbar(
    message: String,
    onUndo: () -> Unit,
    modifier: Modifier = Modifier,
    actionLabel: String = stringResource(R.string.undo),
    onDismiss: (() -> Unit)? = null,
) = QuestSnackbar(message, actionLabel, onUndo, modifier, onDismiss)

@Composable
private fun QuestSnackbar(
    message: String,
    actionLabel: String?,
    onAction: () -> Unit,
    modifier: Modifier = Modifier,
    onDismiss: (() -> Unit)? = null,
) {
    val colors = QuestTheme.colors
    Snackbar(
        // A live region, so screen readers announce the message (it is not shown through a SnackbarHost).
        modifier = modifier.padding(QuestSpacing.medium).semantics { liveRegion = LiveRegionMode.Polite }.testTag("snackbar"),
        containerColor = colors.text,
        contentColor = colors.bg,
        action = actionLabel?.let { label ->
            { TextButton(onClick = onAction, modifier = Modifier.testTag("snackbar-action")) { Text(label, color = MaterialTheme.colorScheme.inversePrimary, style = QuestType.nav) } }
        },
        dismissAction = onDismiss?.let { dismiss ->
            {
                IconButton(onClick = dismiss) {
                    Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.dismiss), tint = colors.bg)
                }
            }
        },
    ) {
        Text(message, style = QuestType.body)
    }
}

/** Where sync stands, as a coloured dot and a short label. */
@Composable
fun SyncPill(sync: SyncState, modifier: Modifier = Modifier) {
    val colors = QuestTheme.colors
    val (color, label) = when (sync) {
        is SyncState.SignedOut -> colors.textFaint to stringResource(R.string.sync_local_only)
        is SyncState.Connecting -> StatusColors.offline to stringResource(R.string.sync_connecting)
        is SyncState.Live -> StatusColors.live to
            if (sync.pending > 0u) stringResource(R.string.sync_syncing, sync.pending.toInt()) else stringResource(R.string.sync_synced)
        is SyncState.Offline -> StatusColors.offline to stringResource(R.string.sync_offline, sync.pending.toInt())
        is SyncState.Failed -> StatusColors.failed to stringResource(R.string.sync_failed)
        is SyncState.SessionExpired -> StatusColors.failed to stringResource(R.string.sync_session_expired)
    }
    Row(
        modifier.testTag("sync-status"),
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.tight),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Dot(color, QuestSpacing.small)
        Text(label, style = QuestType.small, color = colors.textMuted)
    }
}
