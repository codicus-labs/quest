package app.quest.android.ui.settings

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
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType

class AiFeaturesActions(
    val set: (Boolean) -> Unit = {},
    val acceptPrivacy: () -> Unit = {},
    val declinePrivacy: () -> Unit = {},
)

/** The account's opt-in to AI features (off by default); the guest is asked to sign in. */
@Composable
fun AiFeaturesSection(signedIn: Boolean, state: AiFeaturesState, actions: AiFeaturesActions) {
    val colors = QuestTheme.colors
    if (!signedIn) {
        Text(stringResource(R.string.settings_ai_features_guest), style = QuestType.small, color = colors.textMuted, modifier = Modifier.testTag("ai-features-guest"))
        return
    }
    SwitchRow(R.string.settings_ai_features, R.string.settings_ai_features_note, state.enabled, actions.set, "ai-features", enabled = !state.saving)
    if (state.enabled && !state.available) {
        Text(stringResource(R.string.settings_ai_features_unavailable), style = QuestType.small, color = colors.textFaint, modifier = Modifier.testTag("ai-features-unavailable"))
    }
    if (state.askingPrivacy) AiPrivacyDialog(actions)
}

/** Says exactly what AI capture sends, before AI features turn on. */
@Composable
fun AiPrivacyDialog(actions: AiFeaturesActions) {
    AlertDialog(
        onDismissRequest = actions.declinePrivacy,
        title = { Text(stringResource(R.string.ai_privacy_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
                Text(stringResource(R.string.ai_privacy_text), style = QuestType.body, modifier = Modifier.testTag("ai-privacy-text"))
                Text(stringResource(R.string.ai_privacy_note), style = QuestType.small, color = QuestTheme.colors.textFaint)
            }
        },
        confirmButton = { TextButton(onClick = actions.acceptPrivacy, modifier = Modifier.testTag("ai-privacy-accept")) { Text(stringResource(R.string.ai_privacy_accept)) } },
        dismissButton = { TextButton(onClick = actions.declinePrivacy, modifier = Modifier.testTag("ai-privacy-cancel")) { Text(stringResource(R.string.cancel)) } },
        modifier = Modifier.testTag("ai-privacy-dialog"),
    )
}
