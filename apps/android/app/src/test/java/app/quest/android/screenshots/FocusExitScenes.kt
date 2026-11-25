package app.quest.android.screenshots

import app.quest.android.ListSamples
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.navigation.Tab
import app.quest.android.ui.shell.OrganizingHint
import app.quest.android.ui.tracked.TrackedActions
import app.quest.android.ui.tracked.TrackedScreen
import app.quest.android.ui.tracked.TrackedUiState

private val tracked = ListSamples.tracked
private val current = tracked.blocks.last()
private val departure = 14u * 60u + 18u
private val trackedActions = TrackedActions({}, {}, {})

internal val focusExitScenes = listOf(
    Scene("r5-focus", SceneKind.Screen) { NoteFocus() },
    Scene("r5-organizing", SceneKind.Screen) {
        val task = current.copy(endMinute = current.endMinute - 4u, range = "14:00–14:32", running = false)
        val organizing = current.copy(
            entryId = "e5", title = "Organizing", colorHex = null, organizing = true,
            startMinute = task.endMinute, range = "14:32–now",
        )
        val afterOrganize = tracked.copy(blocks = tracked.blocks.dropLast(1) + listOf(task, organizing))
        Shell(Tab.Tracked, ListSamples.playerOrganizing) {
            TrackedScreen(TrackedUiState.Ready(afterOrganize), trackedActions)
        }
    },
    Scene("r5-hint", SceneKind.Screen) {
        val outside = tracked.copy(blocks = tracked.blocks.dropLast(1) + current.copy(outsideSinceMinute = departure))
        Shell(Tab.Tracked, ListSamples.playerRunning, abovePlayer = { OrganizingHint("14:18", {}, {}) }) {
            TrackedScreen(TrackedUiState.Ready(outside), trackedActions)
        }
    },
    Scene("r5-tracked-booked", SceneKind.Screen) {
        val task = current.copy(endMinute = departure, running = false, range = "14:00–14:18")
        val organizing = current.copy(
            entryId = "e5", title = "Organizing", colorHex = null, organizing = true,
            startMinute = departure, range = "14:18–now", rebooked = true,
        )
        val booked = tracked.copy(blocks = tracked.blocks.dropLast(1) + listOf(task, organizing), nowMinute = organizing.endMinute)
        val player = ListSamples.playerOrganizing.copy(timer = "18:00")
        Shell(Tab.Tracked, player, abovePlayer = { UndoSnackbar("Booked as Organizing", {}) }) {
            TrackedScreen(TrackedUiState.Ready(booked), trackedActions)
        }
    },
    Scene("r5-sheet-checklist", SceneKind.Window) { NotesSheet() },
)
