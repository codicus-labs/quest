package app.quest.android.capture

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.Clock
import app.quest.android.core.CoreHolder
import app.quest.android.core.DayLabels
import app.quest.android.core.Outcome
import app.quest.android.core.RepeatRules
import app.quest.android.core.failureMessage
import app.quest.android.settings.CapturePreferences
import app.quest.android.ui.task.RepeatEditorState
import app.quest.android.ui.task.RepeatForm
import java.io.Serializable
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.RepeatCheck
import uniffi.quest_mobile.RepeatWeekday

/** The repeat editor open on one task of the draft. */
data class RepeatEdit(val taskKey: String, val form: RepeatForm) : Serializable

/**
 * What the user typed and edited, in the [SavedStateHandle] so it survives process death: the text, and once
 * structured the draft (small: the core caps the text and the proposal). [filled] says the start text (shared,
 * from quick add or search) was taken once.
 */
data class CaptureInputs(
    val filled: Boolean = false,
    val text: String = "",
    val draft: CaptureDraft? = null,
    /** The item whose fields are open (a quest, task or entry key). */
    val expanded: String? = null,
    val repeat: RepeatEdit? = null,
) : Serializable

/** Why dictation stopped without text; the sheet explains each. */
enum class DictationProblem { NoPermission, NotAvailable, NoMatch, Network, Busy, Other }

/** The dictation sheet while it is not structuring or showing a proposal. */
data class DictationState(
    val listening: Boolean = false,
    /** What the recognizer heard so far, shown live; null while not listening. */
    val partial: String? = null,
    val problem: DictationProblem? = null,
)

sealed interface CaptureUiState {
    data object Loading : CaptureUiState

    data class Dictate(
        val text: String,
        val dictation: DictationState,
        /** The AI is working on the text (up to about a minute); Cancel goes back to editing. */
        val structuring: Boolean,
        val failure: CaptureFailure?,
        /** The once-only privacy note is up; Continue sends the text. */
        val askPrivacy: Boolean,
    ) : CaptureUiState

    data class Proposal(
        val draft: CaptureDraft,
        /** null when the core could not read the draft ([error] says why). */
        val view: CaptureView?,
        val expanded: String?,
        val repeat: RepeatEditorState?,
        /** Why Create did not work, or the view failed. */
        val error: String?,
    ) : CaptureUiState

    /** Everything selected was created: [message] with Undo ([undoLabel]) for [showForMs], then the sheet closes. */
    data class Applied(val message: String, val undoLabel: String, val showForMs: Long) : CaptureUiState

    /** Undo did not work: [message] shows, then the sheet closes. */
    data class UndoFailed(val message: String) : CaptureUiState

    /** Done (Undo worked): the sheet closes. */
    data object Closed : CaptureUiState
}

private const val INPUTS_KEY = "capture.inputs"

/**
 * The most text kept in the saved state: a shared article could exceed the binder's limit when the activity
 * saves its state. Far above what the core accepts, so its own "too long" message still decides.
 */
private const val MAX_KEPT_CHARS = 50_000

private sealed interface CaptureCall {
    data class Done(val draft: CaptureDraft) : CaptureCall

    data class Failed(val failure: CaptureFailure) : CaptureCall
}

/**
 * Dictation and the AI proposal: text in (typed, dictated, shared), `capture` on IO, then the proposal to
 * check, edit and create. Kotlin only changes draft fields with picker values; the core reads the draft
 * ([CaptureView]) after every change. The recognizer itself lives in the UI (it needs the main thread and an
 * activity); it reports here through [onPartial], [onFinal] and [onDictationProblem].
 */
class CaptureViewModel(
    private val core: CoreHolder,
    private val savedState: SavedStateHandle,
    private val preferences: CapturePreferences,
    private val dayLabels: DayLabels,
    private val repeatRules: RepeatRules,
    private val clock: Clock,
    /** BCP 47 tag of the system language (`Locale.getDefault().toLanguageTag()`). */
    private val language: String,
    startText: String?,
) : ViewModel() {
    private val inputs: StateFlow<CaptureInputs> = savedState.getStateFlow(INPUTS_KEY, CaptureInputs())
    private val dictation = MutableStateFlow(DictationState())
    private val transient = MutableStateFlow(Transient())
    private var structuring: Job? = null

    /** What is not worth keeping across process death: progress, failures, the result. */
    private data class Transient(
        val structuring: Boolean = false,
        val failure: CaptureFailure? = null,
        val askPrivacy: Boolean = false,
        val error: String? = null,
        val repeatError: String? = null,
        val applied: CaptureUiState.Applied? = null,
        val undoFailed: String? = null,
        val closed: Boolean = false,
    )

    /** Re-reads the proposal once the core is open (after process death) and when its data changes (sync). */
    private val coreData = merge(core.state.map { }, core.changes)

    val state: StateFlow<CaptureUiState> = combine(inputs, dictation, transient, coreData) { i, d, t, _ -> uiState(i, d, t) }
        .stateIn(viewModelScope, SharingStarted.Eagerly, CaptureUiState.Loading)

    init {
        if (!inputs.value.filled) update { it.copy(filled = true, text = startText.orEmpty().take(MAX_KEPT_CHARS)) }
    }

    private fun uiState(inputs: CaptureInputs, dictation: DictationState, t: Transient): CaptureUiState {
        if (t.closed) return CaptureUiState.Closed
        t.undoFailed?.let { return CaptureUiState.UndoFailed(it) }
        t.applied?.let { return it }
        val draft = inputs.draft ?: return CaptureUiState.Dictate(inputs.text, dictation, t.structuring, t.failure, t.askPrivacy)
        val read = core.read { c, now -> c.captureView(draft, now) }
        val repeat = inputs.repeat?.let { RepeatEditorState(it.form, checkRepeat(it.form), t.repeatError) }
        return CaptureUiState.Proposal(draft, (read as? Outcome.Done)?.value, inputs.expanded, repeat, t.error ?: read.failureMessage())
    }

    private fun update(change: (CaptureInputs) -> CaptureInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    private fun editDraft(change: (CaptureDraft) -> CaptureDraft) {
        transient.value = transient.value.copy(error = null)
        update { it.copy(draft = it.draft?.let(change)) }
    }

    // Dictation

    fun editText(text: String) {
        transient.value = transient.value.copy(failure = null)
        update { it.copy(text = text.take(MAX_KEPT_CHARS)) }
    }

    /** The mic was tapped with permission: the UI starts the recognizer while [DictationState.listening]. */
    fun startListening() {
        dictation.value = DictationState(listening = true, partial = "")
    }

    /** Stop tapped: the recognizer still delivers its final result. */
    fun stopListening() {
        if (dictation.value.listening) dictation.value = DictationState()
    }

    fun onPartial(text: String) {
        if (dictation.value.listening) dictation.value = dictation.value.copy(partial = text)
    }

    /** The recognizer's final text, added to what is there (on its own line, so earlier text stays). */
    fun onFinal(text: String) {
        dictation.value = DictationState()
        if (text.isBlank()) return
        update { inputs -> inputs.copy(text = listOf(inputs.text.trimEnd(), text.trim()).filter { it.isNotEmpty() }.joinToString("\n")) }
    }

    fun onDictationProblem(problem: DictationProblem) {
        dictation.value = DictationState(problem = problem)
    }

    // Structuring

    /** Sends the text, after the once-only privacy note. */
    fun structure() {
        if (inputs.value.text.isBlank() || structuring?.isActive == true) return
        // Stops listening and drops an old mic hint: it no longer applies once the text is sent.
        dictation.value = DictationState()
        // Assigned at once, so a second tap while the preference is read does not send twice.
        structuring = viewModelScope.launch {
            if (preferences.prefs.first().privacyAcknowledged) send() else transient.value = transient.value.copy(askPrivacy = true)
        }
    }

    fun acknowledgePrivacy() {
        if (structuring?.isActive == true) return
        transient.value = transient.value.copy(askPrivacy = false)
        structuring = viewModelScope.launch {
            preferences.setPrivacyAcknowledged()
            send()
        }
    }

    fun dismissPrivacy() {
        transient.value = transient.value.copy(askPrivacy = false)
    }

    private suspend fun send() {
        val text = inputs.value.text
        transient.value = Transient(structuring = true)
        val call = core.readOnIo { c, now ->
            try {
                CaptureCall.Done(c.capture(text, language, now))
            } catch (e: CaptureException) {
                CaptureCall.Failed(e.failure)
            }
        }
        val result = when (call) {
            is Outcome.Done -> call.value
            else -> CaptureCall.Failed(CaptureFailure.Other(call.failureMessage().orEmpty()))
        }
        when (result) {
            is CaptureCall.Done -> {
                transient.value = Transient()
                update { it.copy(draft = result.draft, expanded = null, repeat = null) }
            }
            is CaptureCall.Failed -> transient.value = Transient(failure = result.failure)
        }
    }

    /**
     * Back to the text while the AI works. The network call cannot be interrupted; its answer is dropped.
     */
    fun cancelStructuring() {
        structuring?.cancel()
        structuring = null
        transient.value = Transient()
    }

    /** From the proposal back to the text (Back); the proposal is dropped, the text stays. */
    fun backToText() {
        transient.value = Transient()
        update { it.copy(draft = null, expanded = null, repeat = null) }
    }

    // Proposal

    fun toggleExpanded(key: String) = update { it.copy(expanded = if (it.expanded == key) null else key) }

    fun setQuestSelected(key: String, selected: Boolean) = editDraft { d -> d.withQuest(key) { it.copy(selected = selected) } }

    fun setTaskSelected(key: String, selected: Boolean) = editDraft { d -> d.withTask(key) { it.copy(selected = selected) } }

    fun setWorkSelected(key: String, selected: Boolean) = editDraft { d -> d.withWork(key) { it.copy(selected = selected) } }

    fun editQuestTitle(key: String, title: String) = editDraft { d -> d.withQuest(key) { it.copy(title = title.replace('\n', ' ')) } }

    fun editTaskTitle(key: String, title: String) = editDraft { d -> d.withTask(key) { it.copy(title = title.replace('\n', ' ')) } }

    /** Only a new task's title can change; an existing task keeps its own. */
    fun editWorkTitle(key: String, title: String) = editDraft { d ->
        d.withWork(key) { work ->
            val task = work.task as? DraftWorkTask.New ?: return@withWork work
            work.copy(task = task.copy(title = title.replace('\n', ' ')))
        }
    }

    /** null: "No quest". Picking settles an unsure guess. */
    fun setTaskQuest(key: String, quest: DraftQuestRef?) = editDraft { d -> d.withTask(key) { it.copy(quest = quest, questUncertain = false) } }

    fun setWorkQuest(key: String, quest: DraftQuestRef?) = editDraft { d ->
        d.withWork(key) { work ->
            val task = work.task as? DraftWorkTask.New ?: return@withWork work
            work.copy(task = task.copy(quest = quest))
        }
    }

    /** An ISO day from the picker; null: Someday. */
    fun setTaskDay(key: String, iso: String?) {
        val day = iso?.let { pickedDay(it) ?: return }
        editDraft { d -> d.withTask(key) { it.copy(day = day) } }
    }

    fun setTaskBlockStart(key: String, minute: Int) = editDraft { d -> d.withTask(key) { t -> t.copy(block = t.block?.copy(startMinutes = minute)) } }

    fun setTaskBlockEnd(key: String, minute: Int) = editDraft { d -> d.withTask(key) { t -> t.copy(block = t.block?.copy(endMinutes = minute)) } }

    fun removeTaskBlock(key: String) = editDraft { d -> d.withTask(key) { it.copy(block = null) } }

    fun setWorkDay(key: String, iso: String) {
        val day = pickedDay(iso) ?: return
        editDraft { d -> d.withWork(key) { it.copy(day = day) } }
    }

    fun setWorkStart(key: String, minute: Int) = editDraft { d -> d.withWork(key) { it.copy(startMinutes = minute) } }

    fun setWorkEnd(key: String, minute: Int) = editDraft { d -> d.withWork(key) { it.copy(endMinutes = minute) } }

    private fun pickedDay(iso: String): DraftDay? = try {
        dayLabels.picked(iso).let { DraftDay(it.iso, it.label) }
    } catch (e: MobileException) {
        transient.value = transient.value.copy(error = e.message)
        null
    }

    // Repeat (the task sheet's editor)

    fun openRepeat(key: String) {
        val form = inputs.value.draft?.tasks?.firstOrNull { it.key == key }?.repeat ?: return
        transient.value = transient.value.copy(repeatError = null)
        update { it.copy(repeat = RepeatEdit(key, form)) }
    }

    fun editRepeat(change: (RepeatForm) -> RepeatForm) {
        transient.value = transient.value.copy(repeatError = null)
        update { inputs -> inputs.copy(repeat = inputs.repeat?.let { it.copy(form = change(it.form)) }) }
    }

    fun toggleRepeatWeekday(day: RepeatWeekday) {
        val form = inputs.value.repeat?.form ?: return
        try {
            val next = RepeatForm.of(repeatRules.toggleWeekday(form.toDraft(), day, clock.now()))
            editRepeat { next }
        } catch (e: MobileException) {
            transient.value = transient.value.copy(repeatError = e.message)
        }
    }

    /** Takes the edited rule into the draft; a rule the core rejects stays open with its message. */
    fun saveRepeat() {
        val edit = inputs.value.repeat ?: return
        checkRepeat(edit.form).error?.let {
            transient.value = transient.value.copy(repeatError = it)
            return
        }
        editDraft { d -> d.withTask(edit.taskKey) { it.copy(repeat = edit.form) } }
        update { it.copy(repeat = null) }
    }

    /** "Don't repeat": the task is created once. */
    fun removeRepeat() {
        val edit = inputs.value.repeat ?: return
        editDraft { d -> d.withTask(edit.taskKey) { it.copy(repeat = null) } }
        update { it.copy(repeat = null) }
    }

    fun closeRepeat() = update { it.copy(repeat = null) }

    private fun checkRepeat(form: RepeatForm): RepeatCheck = try {
        repeatRules.check(form.toDraft(), clock.now())
    } catch (e: MobileException) {
        RepeatCheck(shownWeekdays = form.weekdays, monthDayPlaceholder = "", summary = "", error = e.message, starts = null, ends = null)
    }

    /** Creates everything selected, as one change. */
    fun apply() {
        val draft = inputs.value.draft ?: return
        when (val outcome = core.perform { c, now -> c.applyCapture(draft, now) }) {
            is Outcome.Done -> {
                val applied = outcome.value
                transient.value = Transient(applied = CaptureUiState.Applied(applied.message, applied.undoLabel, applied.showForMs))
                // Nothing to restore after this: the next open starts fresh.
                savedState[INPUTS_KEY] = CaptureInputs(filled = true)
            }
            else -> transient.value = transient.value.copy(error = outcome.failureMessage())
        }
    }

    /** The snackbar's Undo: deletes what Create made (what still exists), then the sheet closes. */
    fun undoApplied() {
        if (transient.value.applied == null) return
        transient.value = when (val outcome = core.perform { c, now -> c.undoCapture(now) }) {
            is Outcome.Done -> Transient(closed = true)
            else -> Transient(undoFailed = outcome.failureMessage())
        }
    }
}
