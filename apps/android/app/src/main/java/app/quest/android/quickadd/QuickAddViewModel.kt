package app.quest.android.quickadd

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.DayLabels
import app.quest.android.core.Outcome
import app.quest.android.core.failureMessage
import java.io.Serializable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.QuickAddForm

/**
 * What is typed and chosen, in the [SavedStateHandle] so it survives process death. [filled] says the
 * form's presets (shared text, quest, today) were taken once; after that the inputs are the user's.
 */
data class QuickAddInputs(
    val filled: Boolean = false,
    val title: String = "",
    val description: String = "",
    val questId: String? = null,
    /** ISO day; null: backlog. */
    val day: String? = null,
) : Serializable

sealed interface QuickAddUiState {
    data object Loading : QuickAddUiState

    /** [message] from the core; null: it did not open (the sheet says so). */
    data class Failed(val message: String?) : QuickAddUiState

    data class Ready(
        val form: QuickAddForm,
        val inputs: QuickAddInputs,
        /** The chosen day as the core labels it; null: backlog. */
        val day: DayValue?,
        /** Why the last Add did not work (empty title, quest gone…). */
        val error: String? = null,
    ) : QuickAddUiState

    /** The task was added; the sheet closes. */
    data object Added : QuickAddUiState
}

private const val INPUTS_KEY = "quickadd.inputs"

/** The quick-add sheet (share target, shortcut, tile): title, description, quest and day → `quick_add`. */
class QuickAddViewModel(
    private val core: CoreHolder,
    private val savedState: SavedStateHandle,
    private val dayLabels: DayLabels,
    private val sharedText: String?,
) : ViewModel() {
    private val inputs: StateFlow<QuickAddInputs> = savedState.getStateFlow(INPUTS_KEY, QuickAddInputs())
    private val form = MutableStateFlow<Outcome<QuickAddForm>?>(null)
    private val error = MutableStateFlow<String?>(null)
    private val added = MutableStateFlow(false)

    val state: StateFlow<QuickAddUiState> = combine(form, inputs, error, added, ::uiState)
        .stateIn(viewModelScope, SharingStarted.Eagerly, QuickAddUiState.Loading)

    init {
        viewModelScope.launch {
            if (!core.awaitOpen()) {
                form.value = Outcome.NotReady
                return@launch
            }
            val read = core.read { c, now -> c.quickAddForm(sharedText, now) }
            if (read is Outcome.Done && !inputs.value.filled) {
                val preset = read.value
                update { QuickAddInputs(filled = true, title = preset.title, description = preset.description, questId = preset.quest?.id, day = preset.today.iso) }
            }
            form.value = read
        }
    }

    private fun uiState(form: Outcome<QuickAddForm>?, inputs: QuickAddInputs, error: String?, added: Boolean): QuickAddUiState {
        if (added) return QuickAddUiState.Added
        val loaded = when (form) {
            null -> return QuickAddUiState.Loading
            is Outcome.Done -> form.value
            else -> return QuickAddUiState.Failed(form.failureMessage())
        }
        if (!inputs.filled) return QuickAddUiState.Loading
        return QuickAddUiState.Ready(loaded, inputs, dayValue(loaded, inputs.day), error)
    }

    private fun dayValue(form: QuickAddForm, iso: String?): DayValue? = when (iso) {
        null -> null
        form.today.iso -> form.today
        else -> try {
            dayLabels.picked(iso)
        } catch (_: Exception) {
            DayValue(iso, iso)
        }
    }

    private fun update(change: (QuickAddInputs) -> QuickAddInputs) {
        savedState[INPUTS_KEY] = change(inputs.value)
    }

    fun editTitle(text: String) = update { it.copy(title = text.replace('\n', ' ')) }

    fun editDescription(text: String) = update { it.copy(description = text) }

    /** null: "No quest". */
    fun setQuest(questId: String?) = update { it.copy(questId = questId) }

    /** An ISO day from the picker; null: backlog. */
    fun setDay(day: String?) = update { it.copy(day = day) }

    /** Adds the task; the core checks the title, quest and day. */
    fun add() {
        val current = inputs.value
        val outcome = core.perform { c, now -> c.quickAdd(current.title, current.description, current.questId, current.day, now) }
        error.value = outcome.failureMessage()
        if (outcome is Outcome.Done) added.value = true
    }
}
