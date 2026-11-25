package app.quest.android.screenshots

import app.quest.android.CaptureSamples
import app.quest.android.QuestSamples
import app.quest.android.Samples
import app.quest.android.capture.CaptureActions
import app.quest.android.capture.CaptureContent
import app.quest.android.capture.CaptureFailure
import app.quest.android.capture.CaptureUiState
import app.quest.android.capture.DictationState
import app.quest.android.quickadd.QuickAddActions
import app.quest.android.quickadd.QuickAddContent
import app.quest.android.quickadd.QuickAddInputs
import app.quest.android.quickadd.QuickAddUiState
import app.quest.android.ui.search.SavedSearchMode
import app.quest.android.ui.search.SearchActions
import app.quest.android.ui.search.SearchScreen
import app.quest.android.ui.search.SearchUiState

private const val SAMPLE_TEXT =
    "Tomorrow write the offer for Miller from 9 to 11, part of the website relaunch. New quest tax return, collect receipts every Friday. " +
        "Worked on the relaunch concept from nine to eleven today."

private fun dictate(
    text: String = SAMPLE_TEXT,
    dictation: DictationState = DictationState(),
    structuring: Boolean = false,
    failure: CaptureFailure? = null,
) = CaptureUiState.Dictate(text, dictation, structuring, failure, askPrivacy = false)

private fun proposal(expanded: String? = null, draft: app.quest.android.capture.CaptureDraft = CaptureSamples.draft) =
    CaptureUiState.Proposal(draft, CaptureSamples.view(draft), expanded, repeat = null, error = null)

/** AI capture: dictation and the proposal (and its entries in quick add and search). */
val captureScenes = listOf(
    Scene("capture-dictate", SceneKind.Screen) { SheetFrame { CaptureContent(dictate(), CaptureActions.None) } },
    Scene("capture-listening", SceneKind.Screen) {
        SheetFrame {
            CaptureContent(dictate(text = "Tomorrow write the offer for Miller.", dictation = DictationState(listening = true, partial = "new quest tax return collect")), CaptureActions.None)
        }
    },
    Scene("capture-structuring", SceneKind.Screen) { SheetFrame { CaptureContent(dictate(structuring = true), CaptureActions.None) } },
    Scene("capture-offline", SceneKind.Screen) {
        SheetFrame { CaptureContent(dictate(failure = CaptureFailure.Offline("could not reach your Quest server.")), CaptureActions.None) }
    },
    Scene("capture-proposal", SceneKind.Screen) { SheetFrame { CaptureContent(proposal(), CaptureActions.None) } },
    Scene("capture-proposal-task", SceneKind.Screen) { SheetFrame { CaptureContent(proposal(expanded = "t1"), CaptureActions.None) } },
    Scene("capture-proposal-work", SceneKind.Screen) {
        val draft = CaptureSamples.draft.copy(quests = CaptureSamples.draft.quests.map { it.copy(selected = false) })
        SheetFrame { CaptureContent(proposal(expanded = "w1", draft = draft), CaptureActions.None) }
    },
    Scene("quick-add-mic", SceneKind.Screen) {
        SheetFrame {
            QuickAddContent(
                QuickAddUiState.Ready(Samples.quickAddForm, QuickAddInputs(filled = true, title = "Book the venue", questId = "q1", day = "2026-11-23"), Samples.quickAddForm.today),
                QuickAddActions({}, {}, {}, {}, {}, {}, dictate = {}),
            )
        }
    },
    Scene("search-capture", SceneKind.Screen) {
        SearchScreen(SearchUiState.Ready("", SavedSearchMode.Commands, "Search or jump…", QuestSamples.commands), SearchActions({}, {}, {}, {}, captureWithAi = {}))
    },
)
