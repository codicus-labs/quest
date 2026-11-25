package app.quest.android.screenshots

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.ui.Alignment
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.WorkspaceSamples
import app.quest.android.settings.HomePrefs
import app.quest.android.ui.components.DayGrid
import app.quest.android.ui.components.TaskCardActions
import app.quest.android.ui.home.CompactView
import app.quest.android.ui.home.HomeActions
import app.quest.android.ui.home.HomeInputs
import app.quest.android.ui.task.RepeatEditorState
import app.quest.android.ui.task.RepeatForm
import app.quest.android.ui.task.SheetPage
import app.quest.android.ui.task.TaskSheetActions
import app.quest.android.ui.task.TaskSheetContent
import app.quest.android.ui.task.TaskSheetState
import app.quest.android.ui.theme.QuestTheme
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.HomeData
import uniffi.quest_mobile.RepeatCheck
import uniffi.quest_mobile.RepeatFrequency
import androidx.compose.runtime.Composable
import app.quest.android.Samples
import app.quest.android.settings.Appearance
import app.quest.android.ui.account.AccountActions
import app.quest.android.ui.account.AccountScreen
import app.quest.android.ui.account.AccountUiState
import app.quest.android.ui.components.HoursHeader
import app.quest.android.ui.components.QuestChip
import app.quest.android.ui.components.QuestSheetContent
import app.quest.android.ui.components.SectionHeader
import app.quest.android.ui.components.SyncPill
import app.quest.android.ui.components.TaskCard
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.focus.FocusActions
import app.quest.android.ui.focus.FocusScreen
import app.quest.android.ui.focus.FocusUiState
import app.quest.android.ui.home.HomeScreen
import app.quest.android.ui.home.HomeUiState
import app.quest.android.ui.navigation.DetailScaffold
import app.quest.android.ui.navigation.QuestScaffold
import app.quest.android.ui.navigation.TopDestination
import app.quest.android.ui.settings.SettingsActions
import app.quest.android.ui.settings.SettingsScreen
import app.quest.android.ui.settings.SettingsUiState
import uniffi.quest_mobile.SyncState
import uniffi.quest_mobile.PlanData
import app.quest.android.PlanSamples
import app.quest.android.CalendarSamples
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.remember
import app.quest.android.ui.calendar.CalendarActions
import app.quest.android.ui.calendar.CalendarDragState
import app.quest.android.ui.calendar.CalendarGrid
import app.quest.android.ui.calendar.CalendarScreen
import app.quest.android.ui.calendar.CalendarSheetActions
import app.quest.android.ui.calendar.CalendarSheetContent
import app.quest.android.ui.calendar.CalendarUiState
import app.quest.android.ui.calendar.EntryForm
import app.quest.android.ui.calendar.EntryState
import app.quest.android.ui.calendar.LogWorkForm
import app.quest.android.ui.calendar.LogWorkState
import app.quest.android.ui.calendar.TaskPickerState
import app.quest.android.ui.calendar.calendarDayCount
import uniffi.quest_mobile.CalendarData
import uniffi.quest_mobile.GridDrag
import uniffi.quest_mobile.GridDragKind
import uniffi.quest_mobile.TaskChoices
import app.quest.android.ui.plan.DropLine
import app.quest.android.ui.plan.DropTarget
import app.quest.android.ui.plan.MoveSheet
import app.quest.android.ui.plan.MoveToContent
import app.quest.android.ui.plan.PlanActions
import app.quest.android.ui.plan.PlanDragState
import app.quest.android.ui.plan.PlanInputs
import app.quest.android.ui.plan.PlanRow
import app.quest.android.ui.plan.PlanRowContent
import app.quest.android.ui.plan.PlanScreen
import app.quest.android.ui.plan.PlanUiState
import app.quest.android.ui.plan.dayRows

/**
 * What a scene shows: a whole screen (window-sized), a component strip (as tall as it needs), or a
 * screen with a popup (a menu) open, captured with every window.
 */
enum class SceneKind { Screen, Component, Window }

class Scene(val name: String, val kind: SceneKind, val content: @Composable () -> Unit) {
    override fun toString() = name
}

internal val noAccountActions = AccountActions()
internal val noSettingsActions = SettingsActions(changeWeeklyTarget = {}, setAppearance = {}, setKeepScreenOn = {})

@Composable
private fun Top(destination: TopDestination, content: @Composable () -> Unit) =
    QuestScaffold(destination, onNavigate = {}, onSearch = {}, onAccount = {}, workspaces = WorkspaceSamples.one, content = content)

private val noCardActions = TaskCardActions({}, {}, {})

private fun homeState(
    home: HomeData = Samples.home,
    sync: SyncState = SyncState.Live(0u),
    prefs: HomePrefs = HomePrefs(),
    inputs: HomeInputs = HomeInputs(),
) = HomeUiState.Ready(home, Samples.grid, sync, prefs, inputs)

private fun planState(plan: PlanData = PlanSamples.plan, prefs: HomePrefs = HomePrefs(), inputs: PlanInputs = PlanInputs()) =
    PlanUiState.Ready(plan, prefs, inputs, moveSheet = null, scroll = null)

@Composable
private fun Plan(state: PlanUiState.Ready) = Top(TopDestination.Plan) { PlanScreen(state, PlanActions.None) }

@Composable
private fun Home(state: HomeUiState) = Top(TopDestination.Home) { HomeScreen(state, HomeActions.None) }

/** Focus over the app, as in [app.quest.android.ui.QuestApp]. */
@Composable
private fun Focus(state: FocusUiState) = FocusScreen(state, FocusActions.None, keepScreenOn = false)

private fun repeatState(form: RepeatForm, check: RepeatCheck) =
    TaskSheetState.Open(Samples.detail, Samples.detail.title, Samples.detail.description, SheetPage.Repeat, RepeatEditorState(form, check, null))

/** The sheet's content on the sheet colour (the modal sheet itself is a separate window). */
@Composable
private fun Sheet(state: TaskSheetState.Open, repeats: Boolean = false) {
    val shown = if (repeats) state.copy(detail = state.detail.copy(repeats = true, repeatLabel = "Every month")) else state
    SheetFrame { TaskSheetContent(shown, TaskSheetActions.None) }
}

/** A modal sheet's area: centred and no wider than Material's modal sheet (`BottomSheetDefaults.SheetMaxWidth`, 640 dp), as on a tablet. */
@Composable
internal fun SheetFrame(content: @Composable () -> Unit) {
    Box(Modifier.fillMaxSize().background(QuestTheme.colors.bg), contentAlignment = Alignment.TopCenter) {
        Box(Modifier.widthIn(max = 640.dp).fillMaxSize().background(QuestTheme.colors.card).padding(top = 24.dp)) { content() }
    }
}

@Composable
private fun Calendar(page: CalendarData) = Top(TopDestination.Calendar) {
    CalendarScreen(CalendarUiState.Ready(page, null, null, null), CalendarActions.None)
}

/** The page as many days wide as the form shows (phone 1, tablet a week). */
@Composable
private fun CalendarFitting(change: (CalendarData) -> CalendarData = { it }) = BoxWithConstraints {
    // The tablet's rail takes 80 dp, as in the app.
    Calendar(change(CalendarSamples.page(calendarDayCount(maxWidth - 80.dp))))
}

private fun calendarSheet(logWork: LogWorkState? = null, entry: EntryState? = null, picker: TaskPickerState? = null) =
    CalendarUiState.Ready(CalendarSamples.page(1), logWork, entry, picker)

/** A calendar sheet's content on the sheet colour (the modal sheet itself is a separate window). */
@Composable
private fun CalendarSheet(state: CalendarUiState.Ready) =
    Box(Modifier.fillMaxSize().background(QuestTheme.colors.card).padding(top = 24.dp)) { CalendarSheetContent(state, CalendarSheetActions.None) }

private val logWorkForm = LogWorkForm.of(CalendarSamples.logWorkDraft)

/** Every component and screen with sample data; each is captured light/dark × phone/tablet. */
val scenes = listOf(
    Scene("task-card", SceneKind.Component) {
        (Samples.home.questWork + Samples.home.other).forEach { TaskCard(it, noCardActions) }
        // As in a plan board column: the time and timer wrap before the quest's name shortens.
        TaskCard(Samples.home.questWork.first(), noCardActions, Modifier.width(260.dp))
    },
    Scene("day-grid", SceneKind.Component) { DayGrid(Samples.grid, Modifier.fillMaxWidth().height(560.dp)) },
    Scene("quest-chip", SceneKind.Component) {
        QuestChip(Samples.writing)
        QuestChip(null)
    },
    Scene("hours-header", SceneKind.Component) { HoursHeader("Monday, November 23", "1.5h", "6.2h", 0.31f) },
    Scene("section-header", SceneKind.Component) { SectionHeader("Quest work") },
    Scene("sync-pill", SceneKind.Component) {
        SyncPill(SyncState.SignedOut)
        SyncPill(SyncState.Live(0u))
        SyncPill(SyncState.Offline(3u, 10u))
        SyncPill(SyncState.Failed("server said 401"))
    },
    Scene("bottom-sheet", SceneKind.Component) { QuestSheetContent("New task") { Text("Sheet content goes here.") } },
    Scene("undo-snackbar", SceneKind.Component) {
        UndoSnackbar("Task deleted", onUndo = {})
        UndoSnackbar(Samples.toast.text, onUndo = {}, onDismiss = {})
    },
    Scene("home", SceneKind.Screen) { Home(homeState()) },
    Scene("home-filtered", SceneKind.Screen) {
        Home(homeState(home = Samples.home.copy(questFilter = Samples.fitness, addQuest = null), inputs = HomeInputs(questFilter = Samples.fitness.id)))
    },
    Scene("home-empty", SceneKind.Screen) { Home(homeState(Samples.emptyHome, SyncState.SignedOut)) },
    Scene("home-compact-grid", SceneKind.Screen) { Home(homeState(inputs = HomeInputs(compactView = CompactView.Grid))) },
    Scene("home-filtered-toast", SceneKind.Screen) {
        val home = Samples.home.copy(
            questWork = Samples.home.questWork.filter { it.quest == Samples.writing },
            questFilter = Samples.writing,
            addQuest = null,
            toast = Samples.toast,
        )
        Home(homeState(home, prefs = HomePrefs(showOther = false, addPosition = AddPosition.BOTTOM), inputs = HomeInputs(questFilter = "q1")))
    },
    Scene("home-add-draft", SceneKind.Screen) {
        Home(homeState(prefs = HomePrefs(addPosition = AddPosition.BOTTOM), inputs = HomeInputs(questWorkDraft = "Sketch the storm scene")))
    },
    Scene("focus-running", SceneKind.Screen) { Focus(FocusUiState.Session(Samples.focusSession, Samples.focusSession.title, Samples.focusSession.description, Samples.home.quests)) },
    Scene("focus-paused", SceneKind.Screen) {
        val paused = Samples.pausedSession.copy(quest = null, header = "FOCUS", timer = "36:00", description = "")
        Focus(FocusUiState.Session(paused, paused.title, paused.description, Samples.home.quests))
    },
    Scene("focus-pick", SceneKind.Screen) { Focus(FocusUiState.Pick(Samples.pick.tasks)) },
    Scene("focus-pick-empty", SceneKind.Screen) { Focus(FocusUiState.Pick(emptyList())) },
    Scene("task-sheet", SceneKind.Screen) { Sheet(TaskSheetState.Open(Samples.detail, Samples.detail.title, Samples.detail.description, SheetPage.Details, null)) },
    Scene("repeat-week", SceneKind.Screen) { Sheet(repeatState(RepeatForm.of(Samples.repeatDraft), Samples.repeatCheck)) },
    Scene("repeat-month-error", SceneKind.Screen) {
        val form = RepeatForm.of(Samples.repeatDraft).copy(every = "0", frequency = RepeatFrequency.MONTH, endsOnDate = true, ends = "2026-11-01")
        Sheet(repeatState(form, Samples.repeatCheck.copy(summary = "", error = "Enter an interval from 1 to 999.")), repeats = true)
    },
    Scene("plan", SceneKind.Screen) { Plan(planState()) },
    Scene("plan-filtered-toast", SceneKind.Screen) {
        val plan = PlanSamples.plan.copy(
            days = PlanSamples.plan.days.map { day -> day.copy(questWork = day.questWork.filter { it.quest == Samples.writing }) },
            questFilter = Samples.writing,
            toast = Samples.toast,
        )
        Plan(planState(plan, prefs = HomePrefs(showOther = false), inputs = PlanInputs(questFilter = "q1", drafts = mapOf("2026-11-24" to "Call the printer"))))
    },
    Scene("plan-drop-line", SceneKind.Component) {
        val drag = PlanDragState().apply { target = DropTarget("2026-11-24", "p2", DropLine(PlanRow.cardKey("p2"), above = true)) }
        PlanSamples.days[3].let { day -> dayRows(day, showOther = true) }.forEach { PlanRowContent(it, planState(), drag, PlanActions.None) }
    },
    Scene("plan-move-sheet", SceneKind.Screen) {
        Box(Modifier.fillMaxSize().background(QuestTheme.colors.card).padding(top = 24.dp)) {
            MoveToContent(MoveSheet("p1", "Sketch the contact sheet", PlanSamples.targets), PlanActions.None, onPickDate = {})
        }
    },
    Scene("calendar", SceneKind.Screen) { CalendarFitting() },
    Scene("calendar-3-days", SceneKind.Screen) { Calendar(CalendarSamples.page(3)) },
    Scene("calendar-other-week-toast", SceneKind.Screen) {
        CalendarFitting { it.copy(showsToday = false, toast = Samples.toast.copy(text = "Moved “Morning run”")) }
    },
    Scene("calendar-drag-ghost", SceneKind.Component) {
        val drag = remember {
            CalendarDragState().apply {
                drag = GridDrag(GridDragKind.MOVE, "e1", "2026-11-23", 430f)
                placement = CalendarSamples.placement
            }
        }
        CalendarGrid(CalendarSamples.page(3).days, rememberScrollState(7 * 52), drag, CalendarActions.None.grid, Modifier.height(420.dp))
    },
    Scene("log-work", SceneKind.Screen) { CalendarSheet(calendarSheet(logWork = LogWorkState(logWorkForm, CalendarSamples.logWorkCheck, null))) },
    Scene("log-work-new-task-error", SceneKind.Screen) {
        val form = logWorkForm.copy(newTitle = "Plan the workshop", questId = "q1", startMinute = 14 * 60, endMinute = 13 * 60 + 30)
        CalendarSheet(calendarSheet(logWork = LogWorkState(form, CalendarSamples.logWorkNewTaskError, null)))
    },
    Scene("log-work-existing-task", SceneKind.Screen) {
        CalendarSheet(calendarSheet(logWork = LogWorkState(logWorkForm.copy(taskId = "t2"), CalendarSamples.logWorkExistingTask, null)))
    },
    Scene("log-work-task-picker", SceneKind.Screen) {
        val picker = TaskPickerState("", CalendarSamples.choices, canCreate = true)
        CalendarSheet(calendarSheet(logWork = LogWorkState(logWorkForm, CalendarSamples.logWorkCheck, null), picker = picker))
    },
    Scene("log-work-task-create", SceneKind.Screen) {
        val picker = TaskPickerState("Plan the workshop", TaskChoices(emptyList(), "Create “Plan the workshop”"), canCreate = true)
        CalendarSheet(calendarSheet(logWork = LogWorkState(logWorkForm, CalendarSamples.logWorkCheck, null), picker = picker))
    },
    Scene("entry-editor", SceneKind.Screen) {
        CalendarSheet(calendarSheet(entry = EntryState(EntryForm.of(CalendarSamples.entryDraft), CalendarSamples.entryCheck, null)))
    },
    Scene("entry-editor-over-midnight", SceneKind.Screen) {
        val form = EntryForm.of(CalendarSamples.entryDraft).copy(taskId = "t4", startMinute = 23 * 60, endDay = "2026-11-24", endMinute = 45, endsOtherDay = true)
        CalendarSheet(calendarSheet(entry = EntryState(form, CalendarSamples.entryOverMidnight, null)))
    },
    Scene("entry-editor-end-date-error", SceneKind.Screen) {
        val form = EntryForm.of(CalendarSamples.entryDraft).copy(taskId = "t4", startMinute = 23 * 60, endDay = "2026-11-22", endMinute = 45, endsOtherDay = true)
        CalendarSheet(calendarSheet(entry = EntryState(form, CalendarSamples.entryEndDateError, null)))
    },
    Scene("account-signed-out", SceneKind.Screen) {
        val state = AccountUiState(
            account = null,
            sync = SyncState.SignedOut,
            serverUrlProblem = "http is only allowed for this device and the emulator's host. Use https://.",
        )
        DetailScaffold("Account", onBack = {}) { AccountScreen(state, noAccountActions) }
    },
    Scene("account-signed-in", SceneKind.Screen) {
        DetailScaffold("Account", onBack = {}) { AccountScreen(AccountUiState(Samples.account, SyncState.Live(0u)), noAccountActions) }
    },
    Scene("settings", SceneKind.Screen) {
        DetailScaffold("Settings", onBack = {}) { SettingsScreen(SettingsUiState(Samples.settings, Appearance.Dark), noSettingsActions) }
    },
) + questPickerScenes + questScenes + workspaceScenes + platformScenes + captureScenes + settingsScenes + accountScenes
