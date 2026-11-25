package app.quest.android.screenshots

import androidx.compose.foundation.text.input.TextFieldState
import app.quest.android.ui.components.QuestPickerContent
import app.quest.android.ui.components.QuestSheetContent
import uniffi.quest_mobile.QuestChip as QuestRef

/** Fifteen quests, most recently used first, as the core sends them. */
private val manyQuests = listOf(
    "Podcast" to "#d968a8", "Tax return 2025" to "#62b87a", "Write the book" to "#d9625c", "Fitness" to "#d98a4e",
    "Garden redesign" to "#d9b84a", "Learn Spanish" to "#b0c94e", "Photo archive" to "#4eb8a8", "Bike repair" to "#5c8fd9",
    "Kitchen renovation" to "#8f6ad9", "Website relaunch" to "#5c8fd9", "Family trip" to "#5c8fd9", "Open source" to "#5c8fd9",
    "Reading list" to "#5c8fd9", "Guitar practice" to "#5c8fd9", "Home office" to "#5c8fd9",
).mapIndexed { i, (name, color) -> QuestRef("m$i", name, color) }

/** The quest picker's sheet: unfiltered (the chosen quest checked), filtered, and with nothing matching. */
val questPickerScenes = listOf(
    Scene("quest-picker", SceneKind.Screen) {
        SheetFrame { QuestSheetContent("Quest") { QuestPickerContent(manyQuests, "m1", "No quest", TextFieldState(), {}, autoFocus = false) } }
    },
    Scene("quest-picker-filtered", SceneKind.Screen) {
        SheetFrame { QuestSheetContent("Quest") { QuestPickerContent(manyQuests, "m1", "No quest", TextFieldState("re"), {}, autoFocus = false) } }
    },
    Scene("quest-picker-empty", SceneKind.Screen) {
        SheetFrame { QuestSheetContent("Filter by quest") { QuestPickerContent(manyQuests, null, "All quests", TextFieldState("xyz"), {}, autoFocus = false) } }
    },
)
