package app.quest.android.capture

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.task.RepeatEditorActions
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import app.quest.android.ui.theme.StatusColors

/** Everything the capture sheets can do, wired to [CaptureViewModel] (the mic through the permission flow). */
class CaptureActions(
    val close: () -> Unit,
    val editText: (String) -> Unit,
    /** The mic: asks for permission the first time, then dictates. */
    val dictate: () -> Unit,
    /** Stops dictating; the text heard so far still arrives. */
    val stopDictating: () -> Unit,
    val structure: () -> Unit,
    val cancelStructuring: () -> Unit,
    val acknowledgePrivacy: () -> Unit,
    val dismissPrivacy: () -> Unit,
    val proposal: ProposalActions,
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = CaptureActions({}, {}, {}, {}, {}, {}, {}, {}, ProposalActions.None)
    }
}

class ProposalActions(
    val backToText: () -> Unit,
    val toggleExpanded: (key: String) -> Unit,
    val setQuestSelected: (key: String, Boolean) -> Unit,
    val setTaskSelected: (key: String, Boolean) -> Unit,
    val setWorkSelected: (key: String, Boolean) -> Unit,
    val editQuestTitle: (key: String, String) -> Unit,
    val editTaskTitle: (key: String, String) -> Unit,
    val editWorkTitle: (key: String, String) -> Unit,
    val setTaskQuest: (key: String, DraftQuestRef?) -> Unit,
    val setWorkQuest: (key: String, DraftQuestRef?) -> Unit,
    /** null: Someday. */
    val setTaskDay: (key: String, String?) -> Unit,
    val setTaskBlockStart: (key: String, Int) -> Unit,
    val setTaskBlockEnd: (key: String, Int) -> Unit,
    val removeTaskBlock: (key: String) -> Unit,
    val setWorkDay: (key: String, String) -> Unit,
    val setWorkStart: (key: String, Int) -> Unit,
    val setWorkEnd: (key: String, Int) -> Unit,
    val openRepeat: (key: String) -> Unit,
    val repeat: RepeatEditorActions,
    val apply: () -> Unit,
) {
    companion object {
        val None = ProposalActions(
            {}, {}, { _, _ -> }, { _, _ -> }, { _, _ -> }, { _, _ -> }, { _, _ -> }, { _, _ -> }, { _, _ -> }, { _, _ -> },
            { _, _ -> }, { _, _ -> }, { _, _ -> }, {}, { _, _ -> }, { _, _ -> }, { _, _ -> }, {},
            RepeatEditorActions({}, {}, {}, {}, {}), {},
        )
    }
}

/** The capture sheet over whatever was on screen (its activity is transparent); closing it ends the activity. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CaptureSheet(state: CaptureUiState, actions: CaptureActions) {
    ModalBottomSheet(
        onDismissRequest = actions.close,
        // Full height only: half open, the keyboard would cover the text and the buttons.
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded)),
        containerColor = QuestTheme.colors.card,
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag("capture"),
    ) {
        CaptureContent(state, actions)
    }
}

/** The sheet's inside, without the sheet (screenshots draw it on its own). */
@Composable
fun CaptureContent(state: CaptureUiState, actions: CaptureActions) {
    Column(
        Modifier
            .fillMaxWidth()
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = QuestSpacing.screen)
            .padding(bottom = QuestSpacing.screen)
            .navigationBarsPadding(),
        verticalArrangement = Arrangement.spacedBy(QuestSpacing.large),
    ) {
        when (state) {
            CaptureUiState.Loading, is CaptureUiState.Applied, is CaptureUiState.UndoFailed, CaptureUiState.Closed -> Unit
            is CaptureUiState.Dictate -> DictateContent(state, actions)
            is CaptureUiState.Proposal -> ProposalContent(state, actions.proposal)
        }
    }
}

@Composable
private fun DictateContent(state: CaptureUiState.Dictate, actions: CaptureActions) {
    // Back while the AI works returns to the text; otherwise the sheet closes (its own back handling).
    BackHandler(enabled = state.structuring, onBack = actions.cancelStructuring)
    SheetTitle(stringResource(R.string.capture_title))
    val listening = state.dictation.listening
    OutlinedTextField(
        value = state.text,
        onValueChange = actions.editText,
        label = { Text(stringResource(R.string.capture_text_label)) },
        placeholder = { Text(stringResource(R.string.capture_text_placeholder)) },
        enabled = !state.structuring,
        minLines = 5,
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
        modifier = Modifier.fillMaxWidth().testTag("capture-text"),
    )
    if (listening) Heard(state.dictation.partial.orEmpty())
    state.dictation.problem?.let { Hint(dictationMessage(it), "capture-dictation-problem") }
    state.failure?.let { Hint(failureMessage(it), "capture-failure") }
    if (state.structuring) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium)) {
            CircularProgressIndicator(Modifier.size(24.dp))
            Text(
                stringResource(R.string.capture_structuring),
                style = QuestType.body,
                color = QuestTheme.colors.textMuted,
                modifier = Modifier.weight(1f).semantics { liveRegion = LiveRegionMode.Polite }.testTag("capture-structuring"),
            )
        }
    }
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        if (!state.structuring) MicButton(listening, actions)
        Box(Modifier.weight(1f))
        if (state.structuring) {
            TextButton(onClick = actions.cancelStructuring, modifier = Modifier.testTag("capture-cancel")) { Text(stringResource(R.string.cancel)) }
        } else {
            Button(
                onClick = actions.structure,
                enabled = state.text.isNotBlank(),
                modifier = Modifier.padding(start = QuestSpacing.small).testTag("capture-structure"),
            ) { Text(stringResource(R.string.capture_structure)) }
        }
    }
    if (state.askPrivacy) PrivacyNote(actions)
}

@Composable
private fun MicButton(listening: Boolean, actions: CaptureActions) {
    FilledTonalButton(
        onClick = if (listening) actions.stopDictating else actions.dictate,
        modifier = Modifier.testTag("capture-mic"),
    ) {
        Icon(QuestIcons.Mic, contentDescription = null, modifier = Modifier.size(20.dp))
        Text(
            stringResource(if (listening) R.string.capture_stop else R.string.capture_dictate),
            modifier = Modifier.padding(start = QuestSpacing.small),
        )
    }
}

/** What the recognizer hears, live; read out politely by TalkBack. */
@Composable
private fun Heard(partial: String) {
    Column(Modifier.fillMaxWidth().heightIn(min = 48.dp).semantics { liveRegion = LiveRegionMode.Polite }.testTag("capture-partial")) {
        Text(stringResource(R.string.capture_listening), style = QuestType.caps, color = QuestTheme.colors.accent)
        if (partial.isNotEmpty()) Text(partial, style = QuestType.body, color = QuestTheme.colors.textMuted)
    }
}

/** Shown once, before the first text goes out. */
@Composable
private fun PrivacyNote(actions: CaptureActions) {
    AlertDialog(
        onDismissRequest = actions.dismissPrivacy,
        title = { Text(stringResource(R.string.capture_privacy_title)) },
        text = { Text(stringResource(R.string.capture_privacy_text)) },
        confirmButton = {
            TextButton(onClick = actions.acknowledgePrivacy, modifier = Modifier.testTag("capture-privacy-continue")) {
                Text(stringResource(R.string.capture_privacy_continue))
            }
        },
        dismissButton = { TextButton(onClick = actions.dismissPrivacy) { Text(stringResource(R.string.cancel)) } },
        modifier = Modifier.testTag("capture-privacy"),
    )
}

@Composable
internal fun SheetTitle(title: String) {
    Text(title, style = QuestType.stat, color = QuestTheme.colors.text, modifier = Modifier.semantics { heading() })
}

/** A problem with what to do about it; announced when it appears. */
@Composable
internal fun Hint(message: String, tag: String) {
    Text(
        message,
        style = QuestType.body,
        color = StatusColors.failed,
        modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }.testTag(tag),
    )
}

@Composable
private fun dictationMessage(problem: DictationProblem): String = stringResource(
    when (problem) {
        DictationProblem.NoPermission -> R.string.capture_mic_denied
        DictationProblem.NotAvailable -> R.string.capture_dictation_unavailable
        DictationProblem.NoMatch -> R.string.capture_dictation_no_match
        DictationProblem.Network -> R.string.capture_dictation_network
        DictationProblem.Busy -> R.string.capture_dictation_busy
        DictationProblem.Other -> R.string.capture_dictation_failed
    },
)

@Composable
private fun failureMessage(failure: CaptureFailure): String = when (failure) {
    CaptureFailure.SignedOut -> stringResource(R.string.capture_failed_signed_out)
    CaptureFailure.NotConfigured -> stringResource(R.string.capture_failed_not_configured)
    is CaptureFailure.Offline -> stringResource(R.string.capture_failed_offline)
    CaptureFailure.Unavailable -> stringResource(R.string.capture_failed_unavailable)
    CaptureFailure.RateLimited -> stringResource(R.string.capture_failed_rate_limited)
    CaptureFailure.InvalidResponse -> stringResource(R.string.capture_failed_invalid)
    is CaptureFailure.TextTooLong -> failure.message
    is CaptureFailure.Other -> failure.message
}
