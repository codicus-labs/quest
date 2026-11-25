package app.quest.android.ui.settings

import app.quest.android.FakeCapturePreferences
import app.quest.android.FakeCore
import app.quest.android.FakeFocusPreferences
import app.quest.android.Samples
import app.quest.android.core.ConnectionsException
import app.quest.android.settings.Appearance
import app.quest.android.settings.AppearanceRepository
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.MobileException

private class FakeAppearances : AppearanceRepository {
    override val appearance = MutableStateFlow(Appearance.System)

    override suspend fun set(appearance: Appearance) {
        this.appearance.value = appearance
    }
}

class SettingsViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()
    private val appearances = FakeAppearances()
    private val focus = FakeFocusPreferences()
    private val capture = FakeCapturePreferences()

    private fun vm(scope: CoroutineScope) =
        SettingsViewModel(openedHolder(core, scope, main.dispatcher), appearances, focus, capture)

    @Test
    fun turningAiFeaturesOnAsksWithThePrivacyNoteFirst() = runTest(main.dispatcher) {
        core.account = Samples.account
        val vm = vm(backgroundScope)
        assertFalse(vm.state.value.ai.enabled)

        vm.setAiFeatures(true)
        assertTrue(vm.state.value.ai.askingPrivacy)
        assertFalse(core.calls.any { it.startsWith("ai enabled") })

        vm.acceptAiPrivacy()
        assertFalse(vm.state.value.ai.askingPrivacy)
        assertEquals(listOf("ai enabled true"), core.calls.filter { it.startsWith("ai enabled") })
        assertTrue(vm.state.value.ai.enabled)
        assertTrue(capture.prefs.value.privacyAcknowledged)
    }

    @Test
    fun decliningThePrivacyNoteLeavesAiFeaturesOff() = runTest(main.dispatcher) {
        core.account = Samples.account
        val vm = vm(backgroundScope)
        vm.setAiFeatures(true)
        vm.declineAiPrivacy()
        assertFalse(vm.state.value.ai.askingPrivacy)
        assertFalse(vm.state.value.ai.enabled)
        assertFalse(core.calls.any { it.startsWith("ai enabled") })
    }

    @Test
    fun turningAiFeaturesOffAppliesAtOnce() = runTest(main.dispatcher) {
        core.account = Samples.account.copy(aiEnabled = true, aiFeatures = true)
        val vm = vm(backgroundScope)
        assertTrue(vm.state.value.ai.enabled)
        vm.setAiFeatures(false)
        assertEquals(listOf("ai enabled false"), core.calls.filter { it.startsWith("ai enabled") })
        assertFalse(vm.state.value.ai.enabled)
    }

    @Test
    fun aFailedAiSwitchShowsTheReason() = runTest(main.dispatcher) {
        core.account = Samples.account.copy(aiEnabled = true, aiFeatures = true)
        val vm = vm(backgroundScope)
        core.failNext = MobileException.ServerUnreachable("could not reach your Quest server (timed out).")
        vm.setAiFeatures(false)
        assertEquals("could not reach your Quest server (timed out).", vm.state.value.message)
        assertTrue(vm.state.value.ai.enabled)
        assertFalse(vm.state.value.ai.saving)
    }

    @Test
    fun keepScreenOnIsOnByDefaultAndToggles() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)
        assertEquals(true, vm.state.value.keepScreenOn)

        vm.setKeepScreenOn(false)

        assertEquals(false, focus.prefs.value.keepScreenOn)
        assertEquals(false, vm.state.value.keepScreenOn)
    }

    @Test
    fun showsTheWeeklyTargetAndAppearance() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)

        assertEquals("20h", vm.state.value.settings?.weeklyTargetLabel)
        assertEquals(Appearance.System, vm.state.value.appearance)
    }

    @Test
    fun stepsTheWeeklyTargetByAnHour() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)

        vm.changeWeeklyTarget(1)
        assertEquals(21u * 60u, core.settings.weeklyTargetMinutes)
        assertEquals("21h", vm.state.value.settings?.weeklyTargetLabel)

        vm.changeWeeklyTarget(-1)
        assertEquals(20u * 60u, core.settings.weeklyTargetMinutes)
    }

    @Test
    fun outOfRangeTargetShowsTheCoresMessage() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)
        core.failNext = MobileException.InvalidWeeklyTarget("a weekly target of 0 minutes is out of range. Choose between 1 and 168 hours.")

        vm.changeWeeklyTarget(-30)

        assertEquals("a weekly target of 0 minutes is out of range. Choose between 1 and 168 hours.", vm.state.value.message)
        vm.dismissMessage()
        assertNull(vm.state.value.message)
    }

    @Test
    fun appearanceIsStoredOnTheDevice() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)

        vm.setAppearance(Appearance.Dark)

        assertEquals(Appearance.Dark, appearances.appearance.value)
        assertEquals(Appearance.Dark, vm.state.value.appearance)
    }

    // Connected AI clients

    private fun signedIn() {
        core.account = Samples.account
    }

    @Test
    fun signedOutHidesTheConnections() = runTest(main.dispatcher) {
        val vm = vm(backgroundScope)
        assertFalse(vm.state.value.signedIn)
        assertEquals(AiConnectionsState.Hidden, vm.state.value.connections)
    }

    @Test
    fun loadsTheConnectedClients() = runTest(main.dispatcher) {
        signedIn()
        val vm = vm(backgroundScope)
        assertTrue(vm.state.value.signedIn)
        vm.loadConnections()
        assertEquals(AiConnectionsState.Loaded(Samples.connections), vm.state.value.connections)
    }

    @Test
    fun showsLoadingUntilTheListArrives() = runTest(main.dispatcher) {
        signedIn()
        val vm = vm(backgroundScope)
        var whileLoading: AiConnectionsState? = null
        core.onConnections = { whileLoading = vm.state.value.connections }
        vm.loadConnections()
        assertEquals(AiConnectionsState.Loading, whileLoading)
        assertEquals(AiConnectionsState.Loaded(Samples.connections), vm.state.value.connections)
    }

    @Test
    fun anEmptyListIsLoadedWithoutRows() = runTest(main.dispatcher) {
        signedIn()
        core.connections = emptyList()
        val vm = vm(backgroundScope)
        vm.loadConnections()
        assertEquals(AiConnectionsState.Loaded(emptyList()), vm.state.value.connections)
    }

    @Test
    fun offlineAndOtherFailuresCanBeRetried() = runTest(main.dispatcher) {
        signedIn()
        core.connectionsFailure = ConnectionsException("could not reach your Quest server (timed out).", offline = true)
        val vm = vm(backgroundScope)
        vm.loadConnections()
        assertEquals(AiConnectionsState.Failed("could not reach your Quest server (timed out).", offline = true), vm.state.value.connections)

        core.connectionsFailure = ConnectionsException("the server no longer accepts this session. Sign out and sign in again.")
        vm.loadConnections()
        assertEquals(AiConnectionsState.Failed("the server no longer accepts this session. Sign out and sign in again.", offline = false), vm.state.value.connections)

        core.connectionsFailure = null
        vm.loadConnections()
        assertEquals(AiConnectionsState.Loaded(Samples.connections), vm.state.value.connections)
    }

    @Test
    fun revokeAsksFirstThenRemovesTheRow() = runTest(main.dispatcher) {
        signedIn()
        val vm = vm(backgroundScope)
        vm.loadConnections()
        val claudeCode = Samples.connections[0]

        vm.askRevoke(claudeCode)
        assertEquals(claudeCode, loaded(vm).confirming)
        vm.cancelRevoke()
        assertNull(loaded(vm).confirming)
        assertFalse(core.calls.any { it.startsWith("revoke") })

        vm.askRevoke(claudeCode)
        vm.revoke()
        assertEquals(listOf("revoke c1"), core.calls.filter { it.startsWith("revoke") })
        assertEquals(AiConnectionsState.Loaded(listOf(Samples.connections[1])), vm.state.value.connections)
    }

    @Test
    fun aFailedRevokeKeepsTheRowWithTheReason() = runTest(main.dispatcher) {
        signedIn()
        val vm = vm(backgroundScope)
        vm.loadConnections()
        core.connectionsFailure = ConnectionsException("could not reach your Quest server (timed out).", offline = true)
        vm.askRevoke(Samples.connections[0])
        vm.revoke()
        assertEquals(Samples.connections, loaded(vm).rows)
        assertEquals("could not reach your Quest server (timed out).", loaded(vm).error)
        assertNull(loaded(vm).revoking)
    }

    @Test
    fun revokingOneRevokedElsewhereRemovesItToo() = runTest(main.dispatcher) {
        signedIn()
        val vm = vm(backgroundScope)
        vm.loadConnections()
        core.connectionsFailure = ConnectionsException("this AI client is no longer connected.", gone = true)
        vm.askRevoke(Samples.connections[0])
        vm.revoke()
        assertEquals(listOf(Samples.connections[1]), loaded(vm).rows)
        assertNull(loaded(vm).error)
    }

    @Test
    fun signingOutHidesTheList() = runTest(main.dispatcher) {
        signedIn()
        val holder = openedHolder(core, backgroundScope, main.dispatcher)
        val vm = SettingsViewModel(holder, appearances, focus, capture)
        vm.loadConnections()
        holder.perform { c, _ -> c.signOut() }
        assertEquals(AiConnectionsState.Hidden, vm.state.value.connections)
    }

    private fun loaded(vm: SettingsViewModel) = vm.state.value.connections as AiConnectionsState.Loaded

    @Test
    fun aReloadDuringARevokeWaitsForIt() = runTest(main.dispatcher) {
        signedIn()
        val vm = vm(backgroundScope)
        vm.loadConnections()
        core.onRevoke = { vm.loadConnections() }
        vm.askRevoke(Samples.connections[0])
        vm.revoke()
        assertEquals(AiConnectionsState.Loaded(listOf(Samples.connections[1])), vm.state.value.connections)
        assertEquals(1, core.calls.count { it == "ai-connections" })
    }
}
