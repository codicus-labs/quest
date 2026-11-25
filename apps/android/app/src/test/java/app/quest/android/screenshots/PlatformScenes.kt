package app.quest.android.screenshots

import app.quest.android.Samples
import app.quest.android.quickadd.QuickAddActions
import app.quest.android.quickadd.QuickAddContent
import app.quest.android.quickadd.QuickAddInputs
import app.quest.android.quickadd.QuickAddUiState
import uniffi.quest_mobile.DayValue

private fun quickAdd(inputs: QuickAddInputs, day: DayValue? = Samples.quickAddForm.today, error: String? = null) =
    QuickAddUiState.Ready(Samples.quickAddForm, inputs, day, error)

/** The quick-add sheet (share target, shortcut, tile). */
val platformScenes = listOf(
    Scene("quick-add-shared", SceneKind.Screen) {
        SheetFrame {
            QuickAddContent(
                quickAdd(QuickAddInputs(filled = true, title = "Lighthouse keepers of the North Sea", description = "https://example.com/lighthouses", questId = "q1", day = "2026-11-23")),
                QuickAddActions.None,
            )
        }
    },
    Scene("quick-add-error", SceneKind.Screen) {
        SheetFrame { QuickAddContent(quickAdd(QuickAddInputs(filled = true, questId = null, day = null), day = null, error = "Enter a title."), QuickAddActions.None) }
    },
    Scene("quick-add-picked-day", SceneKind.Screen) {
        SheetFrame {
            QuickAddContent(
                quickAdd(QuickAddInputs(filled = true, title = "Book the venue", questId = "q2", day = "2026-11-27"), day = DayValue("2026-11-27", "Fri, Nov 27")),
                QuickAddActions.None,
            )
        }
    },
)
