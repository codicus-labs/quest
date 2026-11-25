package app.quest.android.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.ConnectionsException
import app.quest.android.core.CoreState
import app.quest.android.settings.Appearance
import app.quest.android.settings.AppearanceRepository
import app.quest.android.settings.CapturePreferences
import app.quest.android.settings.FocusPreferences
import app.quest.android.core.Outcome
import app.quest.android.core.failureMessage
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import uniffi.quest_mobile.AiConnectionRow
import uniffi.quest_mobile.SettingsData

data class SettingsUiState(
    /** Null while the core is still opening. */
    val settings: SettingsData?,
    val appearance: Appearance,
    val message: String? = null,
    /** Keep the screen on while Focus is visible (this device only). */
    val keepScreenOn: Boolean = true,
    /** Signed in: the account's AI setting and connected AI clients can be shown. */
    val signedIn: Boolean = false,
    val ai: AiFeaturesState = AiFeaturesState(),
    val connections: AiConnectionsState = AiConnectionsState.Hidden,
)

/** The account's "AI features" switch (stored on the server, for all devices). */
data class AiFeaturesState(
    /** The account opted in. */
    val enabled: Boolean = false,
    /** The server has an AI provider; without one the entries stay hidden even when [enabled]. */
    val available: Boolean = true,
    /** The change is on its way to the server: the switch waits. */
    val saving: Boolean = false,
    /** The privacy notice before turning them on. */
    val askingPrivacy: Boolean = false,
)

private data class AiLocal(val saving: Boolean = false, val askingPrivacy: Boolean = false)

/** One step of the weekly-target stepper. */
private const val MINUTES_PER_HOUR = 60L

class SettingsViewModel(
    private val core: CoreHolder,
    private val appearances: AppearanceRepository,
    private val focus: FocusPreferences,
    private val capture: CapturePreferences,
) : ViewModel() {
    private val message = MutableStateFlow<String?>(null)
    private val connections = MutableStateFlow<AiConnectionsState>(AiConnectionsState.Hidden)
    private var loading: Job? = null

    private val ai = MutableStateFlow(AiLocal())

    private val local = combine(appearances.appearance, focus.prefs, message) { appearance, focusPrefs, message ->
        SettingsUiState(null, appearance, message, focusPrefs.keepScreenOn)
    }

    val state: StateFlow<SettingsUiState> =
        combine(core.state, local, connections, ai) { coreState, local, connections, ai ->
            val account = (coreState as? CoreState.Ready)?.account
            val signedIn = account != null
            local.copy(
                settings = (coreState as? CoreState.Ready)?.settings,
                signedIn = signedIn,
                ai = AiFeaturesState(account?.aiEnabled == true, account?.aiAvailable ?: true, ai.saving, ai.askingPrivacy && signedIn),
                connections = if (signedIn) connections else AiConnectionsState.Hidden,
            )
        }.stateIn(viewModelScope, SharingStarted.Eagerly, SettingsUiState(null, Appearance.System))

    /** Moves the weekly target by whole hours; the core checks the range and explains violations. */
    fun changeWeeklyTarget(byHours: Int) {
        val current = state.value.settings ?: return
        val minutes = (current.weeklyTargetMinutes.toLong() + byHours * MINUTES_PER_HOUR).coerceAtLeast(0)
        message.value = core.perform { c, _ -> c.setWeeklyTarget(minutes.toUInt()) }.failureMessage()
    }

    fun setAppearance(appearance: Appearance) {
        viewModelScope.launch { appearances.set(appearance) }
    }

    fun setKeepScreenOn(keep: Boolean) {
        viewModelScope.launch { focus.setKeepScreenOn(keep) }
    }

    /** The "AI features" switch: turning on asks with the privacy notice first; off applies at once. */
    fun setAiFeatures(enabled: Boolean) {
        if (ai.value.saving) return
        if (enabled) ai.value = ai.value.copy(askingPrivacy = true) else saveAiFeatures(false)
    }

    /** "Turn on" in the privacy notice; capture then does not ask again on this device. */
    fun acceptAiPrivacy() {
        ai.value = ai.value.copy(askingPrivacy = false)
        viewModelScope.launch { capture.setPrivacyAcknowledged() }
        saveAiFeatures(true)
    }

    fun declineAiPrivacy() {
        ai.value = ai.value.copy(askingPrivacy = false)
    }

    private fun saveAiFeatures(enabled: Boolean) {
        ai.value = ai.value.copy(saving = true)
        message.value = null
        viewModelScope.launch {
            val outcome = core.readOnIo { c, _ -> c.setAiEnabled(enabled) }
            ai.value = ai.value.copy(saving = false)
            message.value = outcome.failureMessage()
        }
    }

    fun dismissMessage() {
        message.value = null
    }

    /** Settings shows while signed in: reads the connected AI clients from the server (on IO). */
    fun loadConnections() {
        // A revoke in flight finishes on the list it started from; a list read meanwhile could still show its client.
        if ((connections.value as? AiConnectionsState.Loaded)?.revoking != null) return
        loading?.cancel()
        connections.value = AiConnectionsState.Loading
        loading = viewModelScope.launch {
            val outcome = core.readOnIo { c, now ->
                try {
                    AiConnectionsState.Loaded(c.aiConnections(now))
                } catch (e: ConnectionsException) {
                    AiConnectionsState.Failed(e.message.orEmpty(), e.offline)
                }
            }
            connections.value = when (outcome) {
                is Outcome.Done -> outcome.value
                else -> AiConnectionsState.Failed(outcome.failureMessage().orEmpty(), offline = false)
            }
        }
    }

    /** Revoke on a row: asks first. */
    fun askRevoke(row: AiConnectionRow) {
        val loaded = connections.value as? AiConnectionsState.Loaded ?: return
        if (loaded.revoking == null) connections.value = loaded.copy(confirming = row, error = null)
    }

    fun cancelRevoke() {
        val loaded = connections.value as? AiConnectionsState.Loaded ?: return
        connections.value = loaded.copy(confirming = null)
    }

    /** The dialog's Revoke: the server cuts the client off; its row goes. */
    fun revoke() {
        val loaded = connections.value as? AiConnectionsState.Loaded ?: return
        val row = loaded.confirming ?: return
        connections.value = loaded.copy(confirming = null, revoking = row.id, error = null)
        viewModelScope.launch {
            val outcome = core.readOnIo { c, _ ->
                try {
                    c.revokeAiConnection(row.id)
                    null
                } catch (e: ConnectionsException) {
                    // Revoked elsewhere already: what the user asked for holds.
                    e.message.orEmpty().takeUnless { e.gone }
                }
            }
            val error = if (outcome is Outcome.Done) outcome.value else outcome.failureMessage()
            val current = connections.value as? AiConnectionsState.Loaded ?: return@launch
            connections.value = if (error == null) {
                current.copy(rows = current.rows.filterNot { it.id == row.id }, revoking = null)
            } else {
                current.copy(revoking = null, error = error)
            }
        }
    }
}
