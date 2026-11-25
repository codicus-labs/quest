package app.quest.android.ui.logwork

import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeCore
import app.quest.android.LogWorkSamples
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test

class LogWorkViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        LogWorkViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved)

    private val LogWorkViewModel.ready get() = state.value!!

    @Test
    fun closedUntilOpenedAndClosedAgain() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertNull(vm.state.value)

        vm.open()
        assertEquals(LogWorkForm.of(LogWorkSamples.draft), vm.ready.logWork?.form)

        vm.close()
        assertNull(vm.state.value)
    }

    @Test
    fun picksOrCreatesATaskChecksLiveAndSaves() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()

        vm.openTaskPicker()
        assertEquals(LogWorkSamples.choices, vm.ready.picker?.choices)
        vm.editTaskQuery("Plan the workshop")
        assertEquals("Create “Plan the workshop”", vm.ready.picker?.choices?.createLabel)
        vm.createTask("Plan the workshop")
        assertNull(vm.ready.picker)
        assertEquals("Plan the workshop", core.logWorkChecks.last().newTitle)

        vm.edit { it.copy(questId = "q1", startMinute = 14 * 60, endMinute = 15 * 60) }
        assertEquals(14u * 60u, core.logWorkChecks.last().startMinute)

        vm.save()
        assertEquals("log null Plan the workshop q1 2026-11-23 840-900", core.calls.last())
        assertNull(vm.state.value)
    }

    @Test
    fun pickingAnExistingTaskDropsATypedTitle() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()
        vm.edit { it.copy(newTitle = "Typed") }

        vm.openTaskPicker()
        vm.pickTask("t2")

        assertEquals("t2", vm.ready.logWork?.form?.taskId)
        assertEquals("", vm.ready.logWork?.form?.newTitle)
    }

    @Test
    fun aRefusedSaveStaysOpenWithTheCoresReasonUntilTheFormChanges() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.open()

        vm.save()
        assertEquals("Choose a task or type a title.", vm.ready.logWork?.saveError)

        vm.edit { it.copy(startMinute = 0) }
        assertNull(vm.ready.logWork?.saveError)
    }

    @Test
    fun theFormSurvivesProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.open()
        vm.edit { it.copy(taskId = "t4", endMinute = 1000) }

        val restored = viewModel(saved)
        assertEquals("t4", restored.ready.logWork?.form?.taskId)
        assertEquals(1000, restored.ready.logWork?.form?.endMinute)
    }
}
