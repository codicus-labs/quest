package app.quest.android.capture

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalAccessibilityManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.quest.android.QuestApplication
import app.quest.android.R
import app.quest.android.core.CoreState
import app.quest.android.core.RustDayLabels
import app.quest.android.core.RustRepeatRules
import app.quest.android.core.SystemClock
import app.quest.android.settings.Appearance
import app.quest.android.ui.components.QuestSnackbarHost
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.task.RepeatEditorActions
import app.quest.android.ui.theme.QuestTheme
import java.util.Locale
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * AI capture over whatever is on screen (transparent, like quick add): the dictation sheet, then the proposal.
 * Started by the "Dictate" shortcut, the share target "Structure with AI" (an exported alias), quick add's mic
 * and the search's action. It shares the process's core; after Create it shows the core's message with Undo, then
 * ends.
 */
class CaptureActivity : ComponentActivity() {
    private val app get() = application as QuestApplication

    private val capture: CaptureViewModel by viewModels {
        viewModelFactory {
            initializer {
                CaptureViewModel(
                    app.core, createSavedStateHandle(), app.capturePreferences, RustDayLabels, RustRepeatRules, SystemClock,
                    Locale.getDefault().toLanguageTag(), startText(intent),
                )
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // Only a fresh "Dictate" start listens by itself, and only with the permission already given.
        val listenNow = savedInstanceState == null && !viaShareAlias(intent) && intent?.action == ACTION_DICTATE && hasMicPermission(this)
        // A shortcut, share or alias can outlive the account's AI setting: then say so and close.
        lifecycleScope.launch {
            if (aiFeaturesOn()) {
                if (listenNow) capture.startListening()
                return@launch
            }
            Toast.makeText(this@CaptureActivity, R.string.capture_ai_off, Toast.LENGTH_LONG).show()
            finish()
        }
        setContent {
            if (!aiAllowed.collectAsStateWithLifecycle().value) return@setContent
            val appearance = app.appearance.appearance.collectAsStateWithLifecycle(null).value ?: return@setContent
            val dark = when (appearance) {
                Appearance.System -> isSystemInDarkTheme()
                Appearance.Light -> false
                Appearance.Dark -> true
            }
            val state by capture.state.collectAsStateWithLifecycle()
            QuestTheme(dark) { CaptureScreen(state, capture, onClose = ::finish) }
        }
    }

    private val aiAllowed = MutableStateFlow(false)

    /** Waits for the open core: the account's AI features (a core that failed to open counts as off). */
    private suspend fun aiFeaturesOn(): Boolean {
        val opened = app.core.state.first { it !is CoreState.Opening }
        return ((opened as? CoreState.Ready)?.account?.aiFeatures == true).also { aiAllowed.value = it }
    }

    companion object {
        /** Opens dictation and starts listening (with permission). */
        const val ACTION_DICTATE = "app.quest.android.action.DICTATE"

        /** Text to start with (quick add's title, the search's query). */
        const val EXTRA_START_TEXT = "app.quest.android.extra.START_TEXT"

        fun intent(context: Context, startText: String? = null, dictate: Boolean = false): Intent =
            Intent(context, CaptureActivity::class.java)
                .setAction(if (dictate) ACTION_DICTATE else Intent.ACTION_MAIN)
                .apply { startText?.takeIf { it.isNotBlank() }?.let { putExtra(EXTRA_START_TEXT, it) } }

        /**
         * Shared text (with its subject first) or the start text. Through the exported share alias only a
         * share counts: other apps can name the alias directly, past its intent filter.
         */
        fun startText(intent: Intent?): String? {
            intent ?: return null
            if (intent.action == Intent.ACTION_SEND) {
                val parts = listOfNotNull(intent.getCharSequenceExtra(Intent.EXTRA_SUBJECT), intent.getCharSequenceExtra(Intent.EXTRA_TEXT))
                return parts.map { it.toString() }.filter { it.isNotBlank() }.joinToString("\n").ifEmpty { null }
            }
            if (viaShareAlias(intent)) return null
            return intent.getStringExtra(EXTRA_START_TEXT)
        }

        /** Started through the exported alias: only its share counts, never our own actions or extras. */
        private fun viaShareAlias(intent: Intent?) = intent?.component?.className == SHARE_ALIAS

        fun hasMicPermission(context: Context) =
            ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
    }
}

/** The sheet with its recognizer, mic permission and the closing snackbar. */
@Composable
private fun CaptureScreen(state: CaptureUiState, capture: CaptureViewModel, onClose: () -> Unit) {
    val context = LocalContext.current
    val dictate = state as? CaptureUiState.Dictate
    val stop = rememberDictation(
        listening = dictate?.dictation?.listening == true,
        language = Locale.getDefault().toLanguageTag(),
        events = DictationEvents(capture::onPartial, capture::onFinal, capture::onDictationProblem, capture::stopListening),
    )
    var explaining by rememberSaveable { mutableStateOf(false) }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) capture.startListening() else capture.onDictationProblem(DictationProblem.NoPermission)
    }
    val askMic = {
        when {
            CaptureActivity.hasMicPermission(context) -> capture.startListening()
            // The first tap explains first; the system dialog comes after Continue.
            else -> explaining = true
        }
    }
    when (state) {
        is CaptureUiState.Applied -> return AppliedSnackbar(state, onUndo = capture::undoApplied, onClose = onClose)
        is CaptureUiState.UndoFailed -> return ClosingSnackbar(state.message, onClose)
        CaptureUiState.Closed -> return LaunchedEffect(Unit) { onClose() }
        else -> {}
    }
    CaptureSheet(state, actions(capture, onClose, askMic, stopDictating = { stop?.stop() ?: capture.stopListening() }))
    if (explaining) {
        AlertDialog(
            onDismissRequest = { explaining = false },
            title = { Text(stringResource(R.string.capture_mic_rationale_title)) },
            text = { Text(stringResource(R.string.capture_mic_rationale)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        explaining = false
                        permission.launch(Manifest.permission.RECORD_AUDIO)
                    },
                    modifier = Modifier.testTag("capture-mic-continue"),
                ) { Text(stringResource(R.string.capture_mic_allow)) }
            },
            dismissButton = { TextButton(onClick = { explaining = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/**
 * After Create: the core's message with Undo, as long as the core says (longer when accessibility services ask
 * for it). A tap elsewhere, the close button or the time running out ends the activity.
 */
@Composable
internal fun AppliedSnackbar(state: CaptureUiState.Applied, onUndo: () -> Unit, onClose: () -> Unit) {
    val accessibility = LocalAccessibilityManager.current
    LaunchedEffect(state) {
        delay(accessibility?.calculateRecommendedTimeoutMillis(state.showForMs, containsIcons = true, containsText = true, containsControls = true) ?: state.showForMs)
        onClose()
    }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
        // A sibling under the snackbar: a tap on the snackbar itself does not reach it.
        Box(Modifier.matchParentSize().pointerInput(Unit) { detectTapGestures { onClose() } })
        UndoSnackbar(
            state.message,
            onUndo = onUndo,
            actionLabel = state.undoLabel,
            onDismiss = onClose,
            modifier = Modifier.navigationBarsPadding().widthIn(max = SnackbarMaxWidth),
        )
    }
}

/** [message] as a snackbar, then the activity ends. */
@Composable
private fun ClosingSnackbar(message: String, onClose: () -> Unit) {
    val snackbar = remember { SnackbarHostState() }
    LaunchedEffect(message) {
        snackbar.showSnackbar(message)
        onClose()
    }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) { QuestSnackbarHost(snackbar, Modifier.navigationBarsPadding()) }
}

private val SnackbarMaxWidth = 560.dp

private fun actions(capture: CaptureViewModel, close: () -> Unit, dictate: () -> Unit, stopDictating: () -> Unit) = CaptureActions(
    close = close,
    editText = capture::editText,
    dictate = dictate,
    stopDictating = stopDictating,
    structure = capture::structure,
    cancelStructuring = capture::cancelStructuring,
    acknowledgePrivacy = capture::acknowledgePrivacy,
    dismissPrivacy = capture::dismissPrivacy,
    proposal = ProposalActions(
        backToText = capture::backToText,
        toggleExpanded = capture::toggleExpanded,
        setQuestSelected = capture::setQuestSelected,
        setTaskSelected = capture::setTaskSelected,
        setWorkSelected = capture::setWorkSelected,
        editQuestTitle = capture::editQuestTitle,
        editTaskTitle = capture::editTaskTitle,
        editWorkTitle = capture::editWorkTitle,
        setTaskQuest = capture::setTaskQuest,
        setWorkQuest = capture::setWorkQuest,
        setTaskDay = capture::setTaskDay,
        setWorkDay = capture::setWorkDay,
        setWorkStart = capture::setWorkStart,
        setWorkEnd = capture::setWorkEnd,
        openRepeat = capture::openRepeat,
        repeat = RepeatEditorActions(capture::editRepeat, capture::toggleRepeatWeekday, capture::saveRepeat, capture::removeRepeat, capture::closeRepeat),
        apply = capture::apply,
    ),
)
