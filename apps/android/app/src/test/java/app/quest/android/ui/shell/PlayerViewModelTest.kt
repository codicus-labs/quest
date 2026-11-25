package app.quest.android.ui.shell

import app.quest.android.FakeCore
import app.quest.android.ListSamples
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class PlayerViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore(player = ListSamples.playerOrganizing)

    private fun TestScope.viewModel() = PlayerViewModel(openedHolder(core, backgroundScope, main.dispatcher))

    @Test
    fun showsTheCoresCardAndGoesBackToTheTask() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(ListSamples.playerOrganizing, vm.state.value)

        assertTrue(vm.start("t5"))
        vm.stop()

        assertEquals(listOf("start t5", "stop"), core.calls)
    }
}
