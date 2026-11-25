package app.quest.android.screenshots

import app.quest.android.Samples
import app.quest.android.settings.Appearance
import app.quest.android.ui.account.AccountBusy
import app.quest.android.ui.account.AccountDialog
import app.quest.android.ui.account.AccountScreen
import app.quest.android.ui.account.AccountUiState
import app.quest.android.ui.navigation.DetailScaffold
import app.quest.android.ui.settings.AiFeaturesState
import app.quest.android.ui.settings.SettingsScreen
import app.quest.android.ui.settings.SettingsUiState
import uniffi.quest_mobile.SyncState

private fun settings(ai: AiFeaturesState, signedIn: Boolean = true) =
    SettingsUiState(Samples.settings, Appearance.System, signedIn = signedIn, ai = ai)

/** Phase 11: signing in through the browser, the guest import, signing out, and the account's AI features. */
val accountScenes = listOf(
    Scene("account-guest", SceneKind.Screen) {
        DetailScaffold("Account", onBack = {}) { AccountScreen(AccountUiState(null, SyncState.SignedOut), noAccountActions) }
    },
    Scene("account-waiting-for-browser", SceneKind.Screen) {
        val state = AccountUiState(null, SyncState.SignedOut, busy = AccountBusy.WaitingForBrowser)
        DetailScaffold("Account", onBack = {}) { AccountScreen(state, noAccountActions) }
    },
    Scene("account-import-guest", SceneKind.Screen) {
        val state = AccountUiState(Samples.account, SyncState.Connecting, dialog = AccountDialog.ImportGuest(Samples.account.displayName))
        DetailScaffold("Account", onBack = {}) { AccountScreen(state, noAccountActions) }
    },
    Scene("account-import-failed", SceneKind.Screen) {
        val failure = "could not reach your Quest server (timed out)."
        val dialog = AccountDialog.ImportGuest(Samples.account.displayName, failure)
        DetailScaffold("Account", onBack = {}) { AccountScreen(AccountUiState(Samples.account, SyncState.Offline(0u, 30u), dialog = dialog), noAccountActions) }
    },
    Scene("account-sign-out-warning", SceneKind.Screen) {
        val warning = "2 changes have not reached the server yet. Signing out deletes them on this device."
        val state = AccountUiState(Samples.account, SyncState.Offline(2u, 30u), dialog = AccountDialog.ConfirmSignOut(warning))
        DetailScaffold("Account", onBack = {}) { AccountScreen(state, noAccountActions) }
    },
    Scene("account-session-expired", SceneKind.Screen) {
        DetailScaffold("Account", onBack = {}) { AccountScreen(AccountUiState(Samples.account, SyncState.SessionExpired), noAccountActions) }
    },
    Scene("settings-ai-off", SceneKind.Screen) {
        DetailScaffold("Settings", onBack = {}) { SettingsScreen(settings(AiFeaturesState()), noSettingsActions) }
    },
    Scene("settings-ai-guest", SceneKind.Screen) {
        DetailScaffold("Settings", onBack = {}) { SettingsScreen(settings(AiFeaturesState(), signedIn = false), noSettingsActions) }
    },
    Scene("settings-ai-unavailable", SceneKind.Screen) {
        DetailScaffold("Settings", onBack = {}) { SettingsScreen(settings(AiFeaturesState(enabled = true, available = false)), noSettingsActions) }
    },
    Scene("settings-ai-privacy", SceneKind.Screen) {
        DetailScaffold("Settings", onBack = {}) { SettingsScreen(settings(AiFeaturesState(askingPrivacy = true)), noSettingsActions) }
    },
)
