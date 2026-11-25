package app.quest.android.ui.account

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import app.quest.android.R
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestType
import app.quest.android.ui.theme.StatusColors

class AccountDialogActions(
    val answerImport: (import: Boolean) -> Unit = {},
    val signOut: () -> Unit = {},
    val cancelSignOut: () -> Unit = {},
)

@Composable
fun AccountDialogs(dialog: AccountDialog, actions: AccountDialogActions) {
    when (dialog) {
        is AccountDialog.ImportGuest -> ImportGuestDialog(dialog, actions.answerImport)
        is AccountDialog.ConfirmSignOut -> SignOutDialog(dialog.warning, actions)
    }
}

/** Asked once after an account's first sign-in on this device while guest data exists; dismissing means No. */
@Composable
private fun ImportGuestDialog(dialog: AccountDialog.ImportGuest, answer: (Boolean) -> Unit) {
    AlertDialog(
        onDismissRequest = { answer(false) },
        title = { Text(stringResource(R.string.account_import_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
                dialog.failure?.let { Text(it, style = QuestType.body, color = StatusColors.failed, modifier = Modifier.testTag("import-failure")) }
                Text(stringResource(R.string.account_import_text, dialog.accountName), modifier = Modifier.testTag("import-text"))
            }
        },
        confirmButton = {
            TextButton(onClick = { answer(true) }, modifier = Modifier.testTag("import-yes")) { Text(stringResource(R.string.account_import_yes)) }
        },
        dismissButton = {
            TextButton(onClick = { answer(false) }, modifier = Modifier.testTag("import-no")) { Text(stringResource(R.string.account_import_no)) }
        },
        modifier = Modifier.testTag("import-dialog"),
    )
}

@Composable
private fun SignOutDialog(warning: String?, actions: AccountDialogActions) {
    AlertDialog(
        onDismissRequest = actions.cancelSignOut,
        title = { Text(stringResource(R.string.account_sign_out_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
                warning?.let { Text(it, style = QuestType.body, color = StatusColors.failed, modifier = Modifier.testTag("sign-out-warning")) }
                Text(stringResource(R.string.account_sign_out_text), style = QuestType.body)
            }
        },
        confirmButton = {
            TextButton(onClick = actions.signOut, modifier = Modifier.testTag("sign-out-confirm")) {
                Text(stringResource(R.string.account_sign_out_confirm), color = StatusColors.failed)
            }
        },
        dismissButton = { TextButton(onClick = actions.cancelSignOut) { Text(stringResource(R.string.cancel)) } },
        modifier = Modifier.testTag("sign-out-dialog"),
    )
}
