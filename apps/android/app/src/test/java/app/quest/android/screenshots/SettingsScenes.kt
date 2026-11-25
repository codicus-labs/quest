package app.quest.android.screenshots

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import app.quest.android.Samples
import app.quest.android.capture.AppliedSnackbar
import app.quest.android.capture.CaptureUiState
import app.quest.android.ui.settings.AiConnectionsActions
import app.quest.android.ui.settings.AiConnectionsSection
import app.quest.android.ui.settings.AiConnectionsState
import app.quest.android.ui.theme.QuestTheme

private val noConnectionActions = AiConnectionsActions()

/** Phase 9: Undo after an AI capture, and the account's connected AI clients in Settings. */
val settingsScenes = listOf(
    Scene("capture-applied", SceneKind.Screen) {
        Box(Modifier.fillMaxSize().background(QuestTheme.colors.bg)) {
            AppliedSnackbar(CaptureUiState.Applied("Created 1 quest, 2 tasks and 1 session", "Undo", 6_000), onUndo = {}, onClose = {})
        }
    },
    Scene("settings-connections", SceneKind.Component) {
        AiConnectionsSection(AiConnectionsState.Loaded(Samples.connections), noConnectionActions)
    },
    Scene("settings-connections-loading", SceneKind.Component) { AiConnectionsSection(AiConnectionsState.Loading, noConnectionActions) },
    Scene("settings-connections-empty", SceneKind.Component) {
        AiConnectionsSection(AiConnectionsState.Loaded(emptyList()), noConnectionActions)
    },
    Scene("settings-connections-offline", SceneKind.Component) {
        AiConnectionsSection(AiConnectionsState.Failed("could not reach your Quest server (timed out).", offline = true), noConnectionActions)
    },
    Scene("settings-connections-revoking", SceneKind.Component) {
        AiConnectionsSection(AiConnectionsState.Loaded(Samples.connections, revoking = "c1"), noConnectionActions)
    },
    Scene("settings-connections-error", SceneKind.Component) {
        val error = "the server no longer accepts this session. Sign out and sign in again."
        AiConnectionsSection(AiConnectionsState.Loaded(Samples.connections, error = error), noConnectionActions)
    },
)
