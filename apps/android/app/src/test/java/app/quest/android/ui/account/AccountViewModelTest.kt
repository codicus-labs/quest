package app.quest.android.ui.account

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeCore
import app.quest.android.Samples
import app.quest.android.core.ServerUrlPolicy
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.SignInResult
import uniffi.quest_mobile.SyncState

class AccountViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()
    private val insecure = "http is only allowed for this device and the emulator's host. Use https://."
    private val policy = ServerUrlPolicy { url -> if (url.startsWith("http://") && !url.contains("127.0.0.1")) insecure else null }
    private val redirects = MutableSharedFlow<String>(extraBufferCapacity = 4)
    private val saved = SavedStateHandle()
    private var switches = 0

    private fun vm(scope: CoroutineScope) =
        AccountViewModel(openedHolder(core, scope, main.dispatcher), policy, saved, redirects) { switches++ }

    /** The pages the view model asked the browser to open. */
    private fun TestScope.browserPages(vm: AccountViewModel): List<String> {
        val pages = mutableListOf<String>()
        backgroundScope.launch { vm.browser.collect { pages += it } }
        return pages
    }

    private fun TestScope.redirect(url: String) {
        redirects.tryEmit(url)
        advanceUntilIdle()
    }

    @Test
    fun signInOpensTheServersPageAndTheRedirectSwitchesToTheAccount() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)
        val pages = browserPages(vm)

        vm.signIn(" https://quest.example.com ")
        advanceUntilIdle()
        assertEquals(listOf("https://quest.example.com"), core.signIns)
        assertEquals(listOf("https://quest.example.com/auth/authorize?client=android"), pages)
        assertEquals(AccountBusy.WaitingForBrowser, vm.state.value.busy)

        redirect("app.quest.android:/oauth2redirect?code=c&state=s")

        assertEquals(Samples.account, vm.state.value.account)
        assertEquals(SyncState.Connecting, vm.state.value.sync)
        assertNull(vm.state.value.busy)
        assertNull(vm.state.value.dialog)
        assertEquals(1, switches)
    }

    @Test
    fun disallowedUrlIsRejectedBeforeTheCoreAndClearedOnEdit() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)

        vm.signIn("http://quest.example.com")

        assertEquals(insecure, vm.state.value.serverUrlProblem)
        assertEquals(emptyList<String>(), core.signIns)
        vm.serverUrlEdited()
        assertNull(vm.state.value.serverUrlProblem)
    }

    @Test
    fun aClosedBrowserEndsTheWaitWithoutAMessage() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)
        browserPages(vm)
        vm.signIn("https://quest.example.com")
        vm.browserReturned()
        assertNull(vm.state.value.busy)
        assertNull(vm.state.value.message)
    }

    @Test
    fun aFailedSignInShowsTheCoresMessage() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)
        vm.signIn("https://quest.example.com")
        core.signInResult = { throw MobileException.SignInCancelled("the sign-in was cancelled.") }

        redirect("app.quest.android:/oauth2redirect?error=access_denied")

        assertEquals("the sign-in was cancelled.", vm.state.value.message)
        assertNull(vm.state.value.account)
        assertNull(vm.state.value.busy)
        assertEquals(0, switches)
        vm.dismissMessage()
        assertNull(vm.state.value.message)
    }

    @Test
    fun theReturnFromSigningOutIsIgnored() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)
        core.signInResult = { SignInResult.NotASignIn }

        redirect("app.quest.android:/oauth2redirect")

        assertNull(vm.state.value.message)
        assertNull(vm.state.value.busy)
        assertEquals(0, switches)
    }

    @Test
    fun guestDataIsOfferedAndImportedOnYes() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)
        core.signInResult = {
            core.account = Samples.account
            SignInResult.SignedIn(offerGuestImport = true, account = Samples.account)
        }
        core.imported = 3u

        redirect("app.quest.android:/oauth2redirect?code=c&state=s")
        assertEquals(AccountDialog.ImportGuest("ada@example.com"), vm.state.value.dialog)

        vm.answerImport(true)
        advanceUntilIdle()
        assertNull(vm.state.value.dialog)
        assertEquals(1, core.calls.count { it == "import guest data" })
        assertNull(vm.state.value.busy)
    }

    @Test
    fun aFailedImportAsksAgainWithTheReason() = runTest(main.dispatcher) {
        saved["account.import_offer"] = "ada@example.com"
        val vm = vm(backgroundScope)
        core.failNext = MobileException.ServerUnreachable("could not reach your Quest server (timed out).")

        vm.answerImport(true)
        advanceUntilIdle()

        assertEquals(AccountDialog.ImportGuest("ada@example.com", "could not reach your Quest server (timed out)."), vm.state.value.dialog)
        vm.answerImport(true)
        advanceUntilIdle()
        assertNull(vm.state.value.dialog)
        assertEquals(1, core.calls.count { it == "import guest data" })
    }

    @Test
    fun aPageNobodyOpensNowIsDroppedNotOpenedLater() = runTest(main.dispatcher) {
        core.account = Samples.account
        val vm = vm(backgroundScope)

        vm.signOut()
        advanceUntilIdle()
        val pages = browserPages(vm)
        advanceUntilIdle()

        assertNull(vm.state.value.account)
        assertEquals(emptyList<String>(), pages)
    }

    @Test
    fun noLeavesTheGuestDataAndTheOfferSurvivesProcessDeath() = runTest(main.dispatcher) {
        saved["account.import_offer"] = "ada@example.com"
        val vm = vm(backgroundScope)
        assertEquals(AccountDialog.ImportGuest("ada@example.com"), vm.state.value.dialog)

        vm.answerImport(false)

        assertNull(vm.state.value.dialog)
        assertEquals(0, core.calls.count { it == "import guest data" })
    }

    @Test
    fun signOutWarnsAboutUnsentChangesThenOpensTheLogoutPage() = runTest(main.dispatcher) {
        core.account = Samples.account
        core.signOutWarning = "2 changes have not reached the server yet. Signing out deletes them on this device."
        val vm = vm(backgroundScope)
        val pages = browserPages(vm)

        vm.askSignOut()
        assertEquals(AccountDialog.ConfirmSignOut(core.signOutWarning), vm.state.value.dialog)
        vm.cancelSignOut()
        assertNull(vm.state.value.dialog)
        assertEquals(0, core.calls.count { it == "sign out" })

        vm.askSignOut()
        vm.signOut()
        advanceUntilIdle()

        assertNull(vm.state.value.dialog)
        assertNull(vm.state.value.account)
        assertEquals(SyncState.SignedOut, vm.state.value.sync)
        assertEquals(listOf("https://quest.example.com/auth/logout?return_to=app"), pages)
        assertEquals(1, switches)
    }

    @Test
    fun anExpiredSessionSignsInAgainAtTheSameServer() = runTest(main.dispatcher) {
        core.account = Samples.account
        core.sync = SyncState.SessionExpired
        val vm = vm(backgroundScope)
        browserPages(vm)

        vm.signInAgain()

        assertEquals(listOf("https://quest.example.com"), core.signIns)
        assertEquals(AccountBusy.WaitingForBrowser, vm.state.value.busy)
    }
}
