package app.quest.android.ui.account

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.quest.android.core.CoreHolder
import app.quest.android.core.CoreState
import app.quest.android.core.Outcome
import app.quest.android.core.ServerUrlPolicy
import app.quest.android.core.failureMessage
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.quest_mobile.Account
import uniffi.quest_mobile.SignInResult
import uniffi.quest_mobile.SyncState

/** A blocking account step in progress. */
enum class AccountBusy { WaitingForBrowser, SigningIn, Importing, SigningOut }

/** The dialog over the Account screen. */
sealed interface AccountDialog {
    /** After an account's first sign-in on this device, with guest data: import it? [failure]: the last try failed. */
    data class ImportGuest(val accountName: String, val failure: String? = null) : AccountDialog

    /** Sign out? [warning]: changes the server has not got yet would be lost. */
    data class ConfirmSignOut(val warning: String?) : AccountDialog
}

data class AccountUiState(
    /** Null for the guest (or while the core is still opening). */
    val account: Account?,
    val sync: SyncState,
    /** Why the entered server URL is not allowed, shown at the field. */
    val serverUrlProblem: String? = null,
    /** A failed action, until dismissed. */
    val message: String? = null,
    val busy: AccountBusy? = null,
    val dialog: AccountDialog? = null,
)

private data class Local(
    val serverUrlProblem: String? = null,
    val message: String? = null,
    val busy: AccountBusy? = null,
    val signOutWarning: String? = null,
    val confirmingSignOut: Boolean = false,
    val importFailure: String? = null,
)

/** Survives process death: the import question must not get lost while the user looks elsewhere. */
private const val IMPORT_OFFER_KEY = "account.import_offer"

/**
 * Signing in through the browser (Custom Tab, RFC 8252) and out, and importing guest data. The browser's answers
 * come from [redirects]; pages to open go out through [browser]. Blocking core calls run on IO. [onSwitched] runs
 * after the active profile changed (signed in as another account, or out), for what lives outside the core.
 */
class AccountViewModel(
    private val core: CoreHolder,
    private val serverUrls: ServerUrlPolicy,
    private val saved: SavedStateHandle,
    redirects: Flow<String>,
    private val onSwitched: () -> Unit = {},
) : ViewModel() {
    private val local = MutableStateFlow(Local())
    /** Rendezvous: a page nobody takes right now (activity stopped) is dropped, never opened later by surprise. */
    private val browserPages = Channel<String>(Channel.RENDEZVOUS)

    /**
     * Pages to open in the browser right away: the sign-in, and after signing out the page that ends the browser's
     * sign-in. Collect only while the activity is started.
     */
    val browser: Flow<String> = browserPages.receiveAsFlow()

    val state: StateFlow<AccountUiState> =
        combine(core.state, local, saved.getStateFlow<String?>(IMPORT_OFFER_KEY, null), ::accountState)
            .stateIn(viewModelScope, SharingStarted.Eagerly, accountState(core.state.value, local.value, saved[IMPORT_OFFER_KEY]))

    init {
        viewModelScope.launch { redirects.collect(::finishSignIn) }
    }

    /** "Sign in": checks the URL, then opens the server's sign-in in the browser. */
    fun signIn(serverUrl: String) {
        val url = serverUrl.trim()
        val problem = serverUrls.problemWith(url)
        local.update { it.copy(serverUrlProblem = problem, message = null) }
        if (problem != null) return
        when (val outcome = core.perform { c, _ -> c.beginSignIn(url) }) {
            is Outcome.Done -> if (browserPages.trySend(outcome.value).isSuccess) {
                local.update { it.copy(busy = AccountBusy.WaitingForBrowser) }
            }
            else -> local.update { it.copy(message = outcome.failureMessage()) }
        }
    }

    /** "Sign in again" after the sign-in expired: the same server. */
    fun signInAgain() {
        state.value.account?.let { signIn(it.serverUrl) }
    }

    /** Back from the browser: without an answer (tab closed) the sign-in is no longer awaited; a late answer still counts. */
    fun browserReturned() = local.update { if (it.busy == AccountBusy.WaitingForBrowser) it.copy(busy = null) else it }

    /** No browser could open the page. */
    fun browserMissing(message: String) = local.update { it.copy(busy = null, message = message) }

    private suspend fun finishSignIn(redirect: String) {
        local.update { it.copy(busy = AccountBusy.SigningIn, message = null) }
        val outcome = core.readOnIo { c, _ -> c.finishSignIn(redirect) }
        local.update { it.copy(busy = null, message = outcome.failureMessage()) }
        val result = (outcome as? Outcome.Done)?.value ?: return
        if (result !is SignInResult.SignedIn) return
        onSwitched()
        if (result.offerGuestImport) saved[IMPORT_OFFER_KEY] = result.account.displayName
    }

    /** The import question's answer; No leaves the guest data where it is. A failed import asks again, with the reason. */
    fun answerImport(import: Boolean) {
        if (!import) {
            saved[IMPORT_OFFER_KEY] = null
            local.update { it.copy(importFailure = null) }
            return
        }
        local.update { it.copy(busy = AccountBusy.Importing, importFailure = null) }
        viewModelScope.launch {
            val outcome = core.readOnIo { c, _ -> c.importGuestData() }
            local.update { it.copy(busy = null, importFailure = outcome.failureMessage()) }
            if (outcome is Outcome.Done) saved[IMPORT_OFFER_KEY] = null
        }
    }

    /** "Sign out": asks first, with a warning if changes would be lost. */
    fun askSignOut() {
        val warning = (core.read { c, _ -> c.signOutWarning() } as? Outcome.Done)?.value
        local.update { it.copy(confirmingSignOut = true, signOutWarning = warning) }
    }

    fun cancelSignOut() = local.update { it.copy(confirmingSignOut = false, signOutWarning = null) }

    fun signOut() {
        local.update { it.copy(confirmingSignOut = false, signOutWarning = null, busy = AccountBusy.SigningOut, message = null) }
        viewModelScope.launch {
            val outcome = core.readOnIo { c, _ -> c.signOut() }
            local.update { it.copy(busy = null, message = outcome.failureMessage()) }
            val logoutPage = (outcome as? Outcome.Done)?.value ?: return@launch
            onSwitched()
            browserPages.trySend(logoutPage)
        }
    }

    /** Clears the URL problem once the user edits the URL. */
    fun serverUrlEdited() = local.update { it.copy(serverUrlProblem = null) }

    fun dismissMessage() = local.update { it.copy(message = null) }
}

private fun accountState(core: CoreState, local: Local, importOffer: String?): AccountUiState {
    val ready = core as? CoreState.Ready
    val dialog = when {
        importOffer != null && local.busy != AccountBusy.Importing -> AccountDialog.ImportGuest(importOffer, local.importFailure)
        local.confirmingSignOut -> AccountDialog.ConfirmSignOut(local.signOutWarning)
        else -> null
    }
    return AccountUiState(
        account = ready?.account,
        sync = ready?.sync ?: SyncState.SignedOut,
        serverUrlProblem = local.serverUrlProblem,
        message = local.message ?: (core as? CoreState.Failed)?.message,
        busy = local.busy,
        dialog = dialog,
    )
}
