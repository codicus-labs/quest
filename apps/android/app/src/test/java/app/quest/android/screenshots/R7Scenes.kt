package app.quest.android.screenshots

import android.graphics.Insets
import android.view.WindowInsets
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import app.quest.android.ListSamples
import app.quest.android.ui.lists.ListActions
import app.quest.android.ui.lists.ListScreen
import app.quest.android.ui.lists.ListUiState
import app.quest.android.ui.lists.QuickAddFields
import app.quest.android.ui.lists.RowActions
import app.quest.android.ui.navigation.Tab
import app.quest.android.ui.shell.MiniPlayer
import app.quest.android.ui.shell.PlayerActions
import org.robolectric.shadows.ShadowDialog
import uniffi.quest_mobile.QuickFields

private val actions = ListActions(RowActions({}, {}, {}, {}), {}, {}, {}, {})
private val fields = QuickAddFields(QuickFields(null, "2026-11-25", null), null, "Nov 25", null)

internal val r7Scenes = listOf(
    Scene("r7-list-player", SceneKind.Screen) {
        Shell(Tab.Today, ListSamples.playerRunning.copy(timer = "18h 38m")) {
            ListScreen(ListUiState.Ready(ListSamples.today, "", fields), actions)
        }
    },
    Scene("r7-quick-add-ime", SceneKind.Window, interact = {
        onNodeWithTag("list-add-open").performClick()
        runOnIdle {
            val decor = checkNotNull(ShadowDialog.getLatestDialog()?.window?.decorView)
            val ime = WindowInsets.Type.ime()
            decor.dispatchApplyWindowInsets(
                WindowInsets.Builder(decor.rootWindowInsets ?: WindowInsets.CONSUMED)
                    .setInsets(ime, Insets.of(0, 0, 0, 250)).setVisible(ime, true).build(),
            )
        }
    }) {
        Shell(Tab.Today, ListSamples.playerRunning) {
            ListScreen(ListUiState.Ready(ListSamples.today, "Review the chapter", fields), actions)
        }
    },
    Scene("r7-player-long", SceneKind.Component) {
        MiniPlayer(ListSamples.playerRunning.copy(timer = "18h 38m"), PlayerActions.None)
        MiniPlayer(ListSamples.playerOrganizing.copy(timer = "99h 59m"), PlayerActions.None)
    },
    Scene("r7-focus-checklist", SceneKind.Screen) { NoteFocus() },
)
