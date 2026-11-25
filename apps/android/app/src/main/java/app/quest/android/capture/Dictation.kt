package app.quest.android.capture

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner

/** What the recognizer reports (to [CaptureViewModel]). */
class DictationEvents(
    val partial: (String) -> Unit,
    val final: (String) -> Unit,
    val problem: (DictationProblem) -> Unit,
    /** The screen stopped (app left): dictation ends without a result. */
    val cancel: () -> Unit,
)

/** Stops the running dictation so that its final text still arrives; null while none runs. */
fun interface StopDictation {
    fun stop()
}

/**
 * Runs Android's speech recognizer while [listening]: on-device when the device has it, else the system's
 * (which may use the network). The recognizer must be created and called on the main thread, which composition
 * effects are; it is destroyed when [listening] ends or the screen goes, and a stopped screen cancels.
 * Returns how to stop it gracefully (final text still comes) while it runs.
 */
@Composable
fun rememberDictation(listening: Boolean, language: String, events: DictationEvents): StopDictation? {
    val context = LocalContext.current
    val currentEvents by rememberUpdatedState(events)
    var running by remember { mutableStateOf<Dictation?>(null) }
    DisposableEffect(listening, language) {
        if (!listening) return@DisposableEffect onDispose {}
        val relay = DictationEvents(
            partial = { currentEvents.partial(it) },
            final = { currentEvents.final(it) },
            problem = { currentEvents.problem(it) },
            cancel = { currentEvents.cancel() },
        )
        val dictation = Dictation(context, language, relay).also { it.start() }
        running = dictation
        onDispose {
            dictation.destroy()
            running = null
        }
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle) {
        val observer = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_STOP) currentEvents.cancel() }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }
    return running?.let { StopDictation(it::stop) }
}

/** One dictation: a recognizer with its listener. Main thread only. */
private class Dictation(private val context: Context, private val language: String, private val events: DictationEvents) {
    private var recognizer: SpeechRecognizer? = null
    private var onDevice = false
    private var finished = false

    fun start() {
        onDevice = SpeechRecognizer.isOnDeviceRecognitionAvailable(context)
        if (!onDevice && !SpeechRecognizer.isRecognitionAvailable(context)) return finish { events.problem(DictationProblem.NotAvailable) }
        listen()
    }

    private fun listen() {
        val created = if (onDevice) SpeechRecognizer.createOnDeviceSpeechRecognizer(context) else SpeechRecognizer.createSpeechRecognizer(context)
        recognizer = created
        created.setRecognitionListener(Listener())
        created.startListening(
            Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
                .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
                .putExtra(RecognizerIntent.EXTRA_LANGUAGE, language)
                .putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true),
        )
    }

    /** Ends listening; the recognizer still delivers what it heard. */
    fun stop() {
        recognizer?.stopListening()
    }

    fun destroy() {
        finished = true
        recognizer?.destroy()
        recognizer = null
    }

    private fun finish(report: () -> Unit) {
        if (finished) return
        destroy()
        report()
    }

    private inner class Listener : RecognitionListener {
        override fun onPartialResults(partialResults: Bundle?) {
            if (!finished) best(partialResults)?.let(events.partial)
        }

        override fun onResults(results: Bundle?) = finish { events.final(best(results).orEmpty()) }

        override fun onError(error: Int) {
            // The on-device model may lack the system language; the system recognizer may still have it.
            if (onDevice && error in LANGUAGE_ERRORS && SpeechRecognizer.isRecognitionAvailable(context)) {
                recognizer?.destroy()
                onDevice = false
                listen()
                return
            }
            finish { events.problem(problemOf(error)) }
        }

        override fun onReadyForSpeech(params: Bundle?) = Unit

        override fun onBeginningOfSpeech() = Unit

        override fun onRmsChanged(rmsdB: Float) = Unit

        override fun onBufferReceived(buffer: ByteArray?) = Unit

        override fun onEndOfSpeech() = Unit

        override fun onEvent(eventType: Int, params: Bundle?) = Unit
    }

    private fun best(results: Bundle?): String? = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)?.firstOrNull()
}

private val LANGUAGE_ERRORS = setOf(SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED, SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE)

internal fun problemOf(error: Int): DictationProblem = when (error) {
    SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> DictationProblem.NoPermission
    SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> DictationProblem.NoMatch
    SpeechRecognizer.ERROR_NETWORK, SpeechRecognizer.ERROR_NETWORK_TIMEOUT, SpeechRecognizer.ERROR_SERVER,
    SpeechRecognizer.ERROR_SERVER_DISCONNECTED,
    -> DictationProblem.Network
    SpeechRecognizer.ERROR_RECOGNIZER_BUSY, SpeechRecognizer.ERROR_TOO_MANY_REQUESTS -> DictationProblem.Busy
    in LANGUAGE_ERRORS -> DictationProblem.NotAvailable
    else -> DictationProblem.Other
}
