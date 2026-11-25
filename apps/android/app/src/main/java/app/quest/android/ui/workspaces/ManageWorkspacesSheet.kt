package app.quest.android.ui.workspaces

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestBottomSheet
import app.quest.android.ui.task.Caps
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.WorkspaceRow
import uniffi.quest_mobile.WorkspacesData

class ManageActions(
    val close: () -> Unit,
    val editNewName: (String) -> Unit,
    val create: () -> Unit,
    val editName: (String, String) -> Unit,
    val rename: (String) -> Unit,
    val askDelete: (String) -> Unit,
    val cancelDelete: () -> Unit,
    val confirmDelete: () -> Unit,
    val restore: (String) -> Unit,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = ManageActions({}, {}, {}, { _, _ -> }, {}, {}, {}, {}, {})
    }
}

/**
 * "Manage workspaces" as a sheet (the desktop's dialog): each live workspace with its name, Rename,
 * Delete and what it holds; deleting asks first and says what goes; "New workspace" with Create
 * (which shows the new one); then the deleted ones with Restore.
 */
@Composable
fun ManageWorkspacesSheet(state: WorkspacesUiState, actions: ManageActions) {
    val ready = state as? WorkspacesUiState.Ready ?: return
    val manage = ready.manage ?: return
    QuestBottomSheet(stringResource(R.string.workspaces_manage), onDismiss = actions.close) { ManageWorkspacesFields(ready.data, manage, actions) }
}

/** The sheet's inside, also drawn on its own for screenshots; it scrolls (large text, keyboard). */
@Composable
fun ManageWorkspacesFields(data: WorkspacesData, manage: ManageInputs, actions: ManageActions) = Column(
    Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).testTag("workspaces-sheet"),
    verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
) {
    data.live.forEach { workspace ->
        val question = manage.confirmDelete?.takeIf { it.workspaceId == workspace.id }
        if (question != null) ConfirmRow(question.text, actions) else LiveRow(workspace, manage.names[workspace.id] ?: workspace.name, actions)
    }
    manage.error?.let {
        Text(
            it,
            style = QuestType.small,
            color = QuestTheme.colors.error,
            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }.testTag("workspace-error"),
        )
    }
    CreateRow(manage.newName, actions)
    if (data.deleted.isNotEmpty()) {
        Caps(stringResource(R.string.workspaces_deleted), Modifier.padding(top = QuestSpacing.small))
        data.deleted.forEach { DeletedRow(it, actions) }
    }
    Spacer(Modifier.padding(bottom = QuestSpacing.small))
}

@Composable
private fun LiveRow(workspace: WorkspaceRow, name: String, actions: ManageActions) {
    Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        OutlinedTextField(
            value = name,
            onValueChange = { actions.editName(workspace.id, it) },
            label = { Text(stringResource(R.string.workspace_name)) },
            singleLine = true,
            textStyle = QuestType.body,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { actions.rename(workspace.id) }),
            modifier = Modifier.fillMaxWidth().testTag("workspace-name-${workspace.id}"),
        )
        Text(
            workspace.summary,
            style = QuestType.small,
            color = QuestTheme.colors.textMuted,
            modifier = Modifier.testTag("workspace-summary-${workspace.id}"),
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small, Alignment.End)) {
            SheetButton(
                stringResource(R.string.workspace_rename),
                "workspace-rename-${workspace.id}",
                description = stringResource(R.string.workspace_rename_named, workspace.name),
            ) { actions.rename(workspace.id) }
            SheetButton(
                stringResource(R.string.delete),
                "workspace-delete-${workspace.id}",
                enabled = workspace.canDelete,
                description = stringResource(R.string.workspace_delete_named, workspace.name),
            ) { actions.askDelete(workspace.id) }
        }
    }
}

@Composable
private fun ConfirmRow(question: String, actions: ManageActions) {
    val colors = QuestTheme.colors
    Column(
        Modifier
            .fillMaxWidth()
            .border(BorderStroke(QuestSpacing.hairline, colors.borderStrong), RoundedCornerShape(QuestSpacing.cardRadius))
            .padding(QuestSpacing.medium)
            .testTag("workspace-confirm"),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        Text(question, style = QuestType.body, color = colors.text, modifier = Modifier.testTag("workspace-delete-question"))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small, Alignment.End)) {
            SheetButton(stringResource(R.string.cancel), "workspace-delete-cancel", onClick = actions.cancelDelete)
            SheetButton(stringResource(R.string.workspace_delete_confirm), "workspace-delete-confirm", danger = true, onClick = actions.confirmDelete)
        }
    }
}

@Composable
private fun CreateRow(newName: String, actions: ManageActions) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
        OutlinedTextField(
            value = newName,
            onValueChange = actions.editNewName,
            placeholder = { Text(stringResource(R.string.workspace_new)) },
            singleLine = true,
            textStyle = QuestType.body,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { actions.create() }),
            modifier = Modifier.weight(1f).testTag("workspace-new"),
        )
        Button(
            onClick = actions.create,
            colors = ButtonDefaults.buttonColors(containerColor = QuestTheme.colors.text, contentColor = QuestTheme.colors.card),
            modifier = Modifier.padding(top = 4.dp).testTag("workspace-create"),
        ) { Text(stringResource(R.string.workspace_create), style = QuestType.bodyMedium) }
    }
}

@Composable
private fun DeletedRow(workspace: WorkspaceRow, actions: ManageActions) {
    Row(Modifier.fillMaxWidth().testTag("workspace-deleted-${workspace.id}"), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(workspace.name, style = QuestType.body, color = QuestTheme.colors.text)
            Text(workspace.summary, style = QuestType.small, color = QuestTheme.colors.textMuted)
        }
        SheetButton(
            stringResource(R.string.workspace_restore),
            "workspace-restore-${workspace.id}",
            description = stringResource(R.string.workspace_restore_named, workspace.name),
        ) { actions.restore(workspace.id) }
    }
}
