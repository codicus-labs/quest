package app.quest.android.ui.calendar

import androidx.lifecycle.SavedStateHandle
import app.quest.android.CalendarSamples
import app.quest.android.FakeCore
import app.quest.android.ui.MainDispatcherRule
import app.quest.android.ui.openedHolder
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import uniffi.quest_mobile.CalendarQuery
import uniffi.quest_mobile.GridDrag
import uniffi.quest_mobile.GridDragKind
import uniffi.quest_mobile.MobileException

class CalendarViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val core = FakeCore()

    private fun TestScope.viewModel(saved: SavedStateHandle = SavedStateHandle()) =
        CalendarViewModel(openedHolder(core, backgroundScope, main.dispatcher), saved)

    private val CalendarViewModel.ready get() = state.value as CalendarUiState.Ready

    private val move = GridDrag(GridDragKind.MOVE, "e1", "2026-11-23", 430f)

    @Test
    fun readsThePageOnlyWhileShownWithTheDaysThatFit() = runTest(main.dispatcher) {
        val vm = viewModel()
        assertEquals(CalendarUiState.Loading, vm.state.value)
        assertTrue(core.calendarReads.isEmpty())

        vm.setShown(true, dayCount = 3)
        assertEquals(CalendarQuery(null, 3u), core.calendarReads.last())
        assertEquals(CalendarSamples.page(1), vm.ready.calendar)

        val reads = core.calendarReads.size
        vm.setShown(false)
        core.home = core.home.copy(todayHours = "2h")
        vm.undo()
        assertEquals(reads, core.calendarReads.size)
        assertEquals(CalendarUiState.Loading, vm.state.value)
    }

    @Test
    fun turnsPagesByTheCoresNeighboursAndBackToToday() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.setShown(true, dayCount = 1)

        vm.next()
        assertEquals(CalendarQuery("2026-11-24", 1u), core.calendarReads.last())
        vm.previous()
        assertEquals(CalendarQuery("2026-11-22", 1u), core.calendarReads.last())

        // The page survives process death.
        val restored = viewModel(saved)
        restored.setShown(true, dayCount = 1)
        assertEquals(CalendarQuery("2026-11-22", 1u), core.calendarReads.last())

        restored.today()
        assertEquals(CalendarQuery(null, 1u), core.calendarReads.last())
    }

    @Test
    fun previewsAreReadsAndAMoveDropSavesWithoutASheet() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        assertEquals(CalendarSamples.placement, vm.preview(move, "2026-11-23", 612.5f))
        assertEquals("preview MOVE e1 2026-11-23 612.5", core.calls.last())

        vm.drop(move, "2026-11-24", 612.5f)
        assertEquals("drop MOVE e1 2026-11-23 430.0 2026-11-24 612.5", core.calls.last())
        assertNull(vm.ready.logWork)
    }

    @Test
    fun aNewSpanDropOpensLogWorkWithItsDraftAndNothingSavesUntilLogWork() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        core.dropDraft = CalendarSamples.logWorkDraft.copy(startMinute = 600u, endMinute = 675u)

        vm.drop(GridDrag(GridDragKind.NEW_SPAN, null, "2026-11-23", 600f), "2026-11-23", 680f)

        val logWork = vm.ready.logWork!!
        assertEquals(600, logWork.form.startMinute)
        assertEquals(675, logWork.form.endMinute)
        assertEquals(CalendarSamples.logWorkCheck, logWork.check)
        assertTrue(core.calls.none { it.startsWith("log ") })
    }

    @Test
    fun logWorkPicksOrCreatesATaskChecksLiveAndSaves() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        vm.openLogWork()

        vm.openTaskPicker()
        assertEquals(CalendarSamples.choices, vm.ready.picker?.choices)
        assertTrue(vm.ready.picker!!.canCreate)
        vm.editTaskQuery("Plan the workshop")
        assertEquals("Create “Plan the workshop”", vm.ready.picker?.choices?.createLabel)
        vm.createTask("Plan the workshop")
        assertNull(vm.ready.picker)
        assertEquals("Plan the workshop", core.logWorkChecks.last().newTitle)

        vm.editLogWork { it.copy(questId = "q1", startMinute = 14 * 60, endMinute = 15 * 60) }
        assertEquals(14u * 60u, core.logWorkChecks.last().startMinute)

        vm.saveLogWork()
        assertEquals("log null Plan the workshop q1 2026-11-23 840-900", core.calls.last())
        assertNull(vm.ready.logWork)
    }

    @Test
    fun pickingAnExistingTaskDropsATypedTitle() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        vm.openLogWork()
        vm.editLogWork { it.copy(newTitle = "Typed") }

        vm.openTaskPicker()
        vm.pickTask("t2")

        assertEquals("t2", vm.ready.logWork?.form?.taskId)
        assertEquals("", vm.ready.logWork?.form?.newTitle)
    }

    @Test
    fun aRefusedSaveStaysOpenWithTheCoresReasonUntilTheFormChanges() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        vm.openLogWork()

        vm.saveLogWork()
        assertEquals("Choose a task or type a title.", vm.ready.logWork?.saveError)

        vm.editLogWork { it.copy(startMinute = 0) }
        assertNull(vm.ready.logWork?.saveError)
    }

    @Test
    fun theLogWorkFormSurvivesProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.setShown(true)
        vm.openLogWork()
        vm.editLogWork { it.copy(taskId = "t4", endMinute = 1000) }

        val restored = viewModel(saved)
        restored.setShown(true)
        assertEquals("t4", restored.ready.logWork?.form?.taskId)
        assertEquals(1000, restored.ready.logWork?.form?.endMinute)
    }

    @Test
    fun theEntryEditorEditsSavesAndDeletes() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        vm.openEntry("e1")
        assertEquals(CalendarSamples.entryCheck, vm.ready.entry?.check)

        vm.openTaskPicker()
        assertTrue(!vm.ready.picker!!.canCreate)
        vm.pickTask(null)
        vm.editEntry { it.copy(endDay = "2026-11-24", endMinute = 45, endsOtherDay = true) }
        vm.saveEntry()
        assertEquals("save-entry e1 null 2026-11-23 420-45 true", core.calls.last())
        assertNull(vm.ready.entry)

        vm.openEntry("e1")
        vm.deleteEntry()
        assertEquals("delete-entry e1", core.calls.last())
        assertNull(vm.ready.entry)
    }

    @Test
    fun aRunningEntryStoppedElsewhereReloadsTheEditorInsteadOfFailing() = runTest(main.dispatcher) {
        // Running: the draft's end is its start until the timer stops.
        val running = CalendarSamples.entryDraft.copy(endMinute = 7u * 60u)
        core.entries["e1"] = running
        core.entryCheck = CalendarSamples.entryCheck.copy(running = true, endLabel = "", duration = "1h")
        val vm = viewModel()
        vm.setShown(true)
        vm.openEntry("e1")
        assertTrue(vm.ready.entry!!.form.running)

        // Another device stops the timer at 08:15 (the page changes with it).
        core.entries["e1"] = CalendarSamples.entryDraft
        core.entryCheck = CalendarSamples.entryCheck
        core.calendar = core.calendar.copy(weekHours = "6.5h")
        vm.undo()

        val entry = vm.ready.entry!!
        assertEquals(EntryForm.of(CalendarSamples.entryDraft), entry.form)
        assertEquals(null, entry.check.error)
        assertNull(vm.message.value)
    }

    @Test
    fun anEntryDeletedElsewhereClosesTheEditor() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        vm.openEntry("e1")

        // Sync deletes it: the entry is gone and its block leaves the page.
        core.entries.clear()
        core.calendar = core.calendar.copy(weekHours = "5h")
        vm.undo()

        assertNull(vm.ready.entry)

        // Closed, not hidden: the entry coming back (Undo elsewhere) doesn't reopen it.
        core.entries["e1"] = CalendarSamples.entryDraft
        core.calendar = core.calendar.copy(weekHours = "6.2h")
        vm.undo()
        assertNull(vm.ready.entry)
    }

    @Test
    fun aFailedCheckKeepsTheEditorOpenWithTheReasonAndNoMessage() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)
        vm.openEntry("e1")

        core.failNext = MobileException.Storage("disk full. Free some space.")
        vm.editEntry { it.copy(startMinute = 400) }
        assertEquals("disk full. Free some space.", vm.ready.sheetError)
        assertNull(vm.message.value)

        // Not closed: the next check shows the edited form again.
        vm.editEntry { it.copy(startMinute = 410) }
        assertEquals(410, vm.ready.entry?.form?.startMinute)
        assertNull(vm.ready.sheetError)
    }

    @Test
    fun theEntryEditorSurvivesProcessDeath() = runTest(main.dispatcher) {
        val saved = SavedStateHandle()
        val vm = viewModel(saved)
        vm.setShown(true)
        vm.openEntry("e1")
        vm.editEntry { it.copy(endMinute = 600) }

        val restored = viewModel(saved)
        restored.setShown(true)
        assertEquals(600, restored.ready.entry?.form?.endMinute)
    }

    @Test
    fun anUnknownEntryDoesNotOpenTheEditor() = runTest(main.dispatcher) {
        val vm = viewModel()
        vm.setShown(true)

        vm.openEntry("gone")

        assertNull(vm.ready.entry)
        assertEquals("This time entry is gone.", vm.message.value)
    }
}
