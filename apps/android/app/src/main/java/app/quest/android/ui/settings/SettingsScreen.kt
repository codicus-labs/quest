package app.quest.android.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedIconButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import app.quest.android.R
import app.quest.android.settings.Appearance
import app.quest.android.ui.account.FormMaxWidth
import app.quest.android.ui.components.SectionHeader
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import app.quest.android.ui.theme.StatusColors

class SettingsActions(
    val changeWeeklyTarget: (byHours: Int) -> Unit,
    val setAppearance: (Appearance) -> Unit,
    val setKeepScreenOn: (Boolean) -> Unit,
    val ai: AiFeaturesActions = AiFeaturesActions(),
    val connections: AiConnectionsActions = AiConnectionsActions(),
)

/** Weekly target (synced); appearance and Focus (this device only); AI features and connected AI clients (account). */
@Composable
fun SettingsScreen(state: SettingsUiState, actions: SettingsActions) {
    // Each visit (and each sign-in) reads the list afresh: clients connect elsewhere.
    LaunchedEffect(state.signedIn) { if (state.signedIn) actions.connections.reload() }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.widthIn(max = FormMaxWidth).fillMaxSize().verticalScroll(rememberScrollState()).padding(QuestSpacing.screen).testTag("settings"),
            verticalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
        ) {
            SectionHeader(stringResource(R.string.settings_weekly_target))
            WeeklyTarget(state, actions.changeWeeklyTarget)
            state.message?.let { Text(it, style = QuestType.body, color = StatusColors.failed, modifier = Modifier.testTag("settings-message")) }
            SectionHeader(stringResource(R.string.settings_appearance), Modifier.padding(top = QuestSpacing.medium))
            AppearancePicker(state.appearance, actions.setAppearance)
            Text(stringResource(R.string.settings_appearance_note), style = QuestType.small, color = QuestTheme.colors.textFaint)
            SectionHeader(stringResource(R.string.settings_focus), Modifier.padding(top = QuestSpacing.medium))
            // Keep the screen on while Focus is visible.
            SwitchRow(R.string.settings_keep_screen_on, R.string.settings_keep_screen_on_note, state.keepScreenOn, actions.setKeepScreenOn, "keep-screen-on")
            SectionHeader(stringResource(R.string.settings_ai_features), Modifier.padding(top = QuestSpacing.medium))
            AiFeaturesSection(state.signedIn, state.ai, actions.ai)
            AiConnectionsSection(state.connections, actions.connections)
        }
    }
}

@Composable
private fun WeeklyTarget(state: SettingsUiState, change: (Int) -> Unit) {
    val colors = QuestTheme.colors
    val settings = state.settings
    val less = stringResource(R.string.settings_hour_less)
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(settings?.weeklyTargetLabel ?: "…", style = QuestType.stat, color = colors.text, modifier = Modifier.testTag("weekly-target"))
            Text(stringResource(R.string.settings_hours_per_week), style = QuestType.small, color = colors.textMuted)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
            OutlinedIconButton(onClick = { change(-1) }, enabled = settings != null, modifier = Modifier.testTag("weekly-target-down")) {
                Text("−", style = QuestType.stat, modifier = Modifier.semantics { contentDescription = less })
            }
            OutlinedIconButton(onClick = { change(1) }, enabled = settings != null, modifier = Modifier.testTag("weekly-target-up")) {
                Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.settings_hour_more))
            }
        }
    }
}

/** A setting with its note; the whole row toggles. */
@Composable
internal fun SwitchRow(title: Int, note: Int, checked: Boolean, onChange: (Boolean) -> Unit, tag: String, enabled: Boolean = true) {
    val colors = QuestTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = QuestSpacing.touchTarget)
            .toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onChange)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(stringResource(title), style = QuestType.body, color = colors.text)
            Text(stringResource(note), style = QuestType.small, color = colors.textMuted)
        }
        Switch(checked = checked, onCheckedChange = null, enabled = enabled)
    }
}

@Composable
private fun AppearancePicker(selected: Appearance, onSelect: (Appearance) -> Unit) {
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
        Appearance.entries.forEachIndexed { index, appearance ->
            SegmentedButton(
                selected = appearance == selected,
                onClick = { onSelect(appearance) },
                shape = SegmentedButtonDefaults.itemShape(index, Appearance.entries.size),
                modifier = Modifier.testTag("appearance-${appearance.name.lowercase()}"),
            ) {
                Text(stringResource(appearanceLabel(appearance)))
            }
        }
    }
}

private fun appearanceLabel(appearance: Appearance) = when (appearance) {
    Appearance.System -> R.string.appearance_system
    Appearance.Light -> R.string.appearance_light
    Appearance.Dark -> R.string.appearance_dark
}
