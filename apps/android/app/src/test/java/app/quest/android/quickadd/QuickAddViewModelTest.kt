package app.quest.android.quickadd

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.Samples
import app.quest.android.core.CoreHolder
import app.quest.android.core.DayLabels
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.MobileException

@OptIn(ExperimentalCoroutinesApi::class)
class QuickAddViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()
    private val savedState = SavedStateHandle()
    private val labels = DayLabels { iso -> DayValue(iso, "Label $iso") }

    private fun TestScope.viewModel(shared: String? = null, holder: CoreHolder = openedHolder(core, backgroundScope, main.dispatcher)) =
        QuickAddViewModel(holder, savedState, labels, shared)

    private fun QuickAddViewModel.ready() = state.value as QuickAddUiState.Ready

    @Test
    fun theFormsPresetsFillTheSheet() = runTest(main.dispatcher) {
        core.quickAddForm = Samples.quickAddForm.copy(title = "Read this", description = "https://example.com")
        val vm = viewModel(shared = "Read this\nhttps://example.com")

        assertEquals(listOf("Read this\nhttps://example.com"), core.quickAddReads)
        val inputs = vm.ready().inputs
        assertEquals("Read this", inputs.title)
        assertEquals("https://example.com", inputs.description)
        assertEquals("q1", inputs.questId)
        assertEquals(DayValue("2026-11-23", "Mon, Nov 23"), vm.ready().day)
    }

    @Test
    fun addsWithWhatWasTypedAndChosen() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.editTitle("Buy\nstamps")
        vm.editDescription("Two books")
        vm.setQuest("q2")
        vm.setDay("2026-11-27")
        assertEquals(DayValue("2026-11-27", "Label 2026-11-27"), vm.ready().day)

        vm.add()

        assertEquals(listOf("quick-add Buy stamps|Two books|q2|2026-11-27"), core.calls)
        assertEquals(QuickAddUiState.Added, vm.state.value)
    }

    @Test
    fun noQuestAndTheBacklog() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.editTitle("Someday")
        vm.setQuest(null)
        vm.setDay(null)
        assertNull(vm.ready().day)

        vm.add()

        assertEquals(listOf("quick-add Someday||null|null"), core.calls)
    }

    @Test
    fun anEmptyTitleStaysOpenWithTheCoresMessage() = runTest(main.dispatcher) {
        val vm = viewModel()

        vm.add()

        assertEquals("Enter a title.", vm.ready().error)
        vm.editTitle("Now")
        vm.add()
        assertEquals(QuickAddUiState.Added, vm.state.value)
    }

    @Test
    fun aGoneQuestShowsTheReason() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.editTitle("Task")
        core.failNext = MobileException.UnknownQuest("That quest no longer exists.")

        vm.add()

        assertEquals("That quest no longer exists.", vm.ready().error)
    }

    @Test
    fun inputsSurviveProcessDeathAndThePresetsDoNotOverwriteThem() = runTest(main.dispatcher) {
        viewModel().apply {
            editTitle("Typed before death")
            setDay(null)
        }
        core.quickAddForm = Samples.quickAddForm.copy(title = "Shared title")

        val restored = viewModel(shared = "Shared title")

        assertEquals("Typed before death", restored.ready().inputs.title)
        assertNull(restored.ready().day)
    }

    @Test
    fun waitsForACoreThatIsStillOpening() = runTest(main.dispatcher) {
        val io = StandardTestDispatcher(testScheduler)
        val vm = viewModel(holder = CoreHolder(backgroundScope, io, FakeClock()) { core })
        assertEquals(QuickAddUiState.Loading, vm.state.value)

        vm.state.first { it is QuickAddUiState.Ready }

        assertEquals("q1", vm.ready().inputs.questId)
    }
}
