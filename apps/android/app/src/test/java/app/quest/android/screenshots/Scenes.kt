package app.quest.android.screenshots

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.ui.Alignment
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.quest.android.ui.components.TaskCardActions
import app.quest.android.ui.task.RepeatEditorState
import app.quest.android.ui.task.RepeatForm
import app.quest.android.ui.task.SheetPage
import app.quest.android.ui.task.TaskSheetActions
import app.quest.android.ui.task.TaskSheetContent
import app.quest.android.ui.task.TaskSheetState
import app.quest.android.ui.theme.QuestTheme
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
import app.quest.android.ui.navigation.DetailScaffold
import app.quest.android.ui.navigation.Tab
import app.quest.android.ui.shell.TodaySegment
import app.quest.android.ui.settings.SettingsActions
import app.quest.android.ui.settings.SettingsScreen
import app.quest.android.ui.settings.SettingsUiState
import uniffi.quest_mobile.SyncState
import app.quest.android.PlanSamples
import app.quest.android.ui.lists.RowActions
import app.quest.android.ui.liftAndDrag
import androidx.compose.ui.test.junit4.ComposeTestRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToKey
import app.quest.android.LogWorkSamples
import app.quest.android.ui.logwork.LogWorkActions
import app.quest.android.ui.logwork.LogWorkForm
import app.quest.android.ui.logwork.LogWorkSheetContent
import app.quest.android.ui.logwork.LogWorkSheetState
import app.quest.android.ui.logwork.LogWorkState
import app.quest.android.ui.logwork.TaskPickerState
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.TaskChoices
import app.quest.android.ui.plan.PlanActions
import app.quest.android.ui.plan.PlanScreen
import app.quest.android.ui.plan.PlanUiState

/**
 * What a scene shows: a whole screen (window-sized), a component strip (as tall as it needs), or a
 * screen with a popup (a menu) open, captured with every window.
 */
enum class SceneKind { Screen, Component, Window }

class Scene(
    val name: String,
    val kind: SceneKind,
    val interact: ComposeTestRule.() -> Unit = {},
    val content: @Composable () -> Unit,
) {
    override fun toString() = name
}

internal val noAccountActions = AccountActions()
internal val noSettingsActions = SettingsActions(changeWeeklyTarget = {}, setAppearance = {}, setKeepScreenOn = {})

private val noCardActions = TaskCardActions({}, {}, {})

@Composable
private fun Plan() = Shell(Tab.Today, leading = { TodaySegment(showPlan = true, onChange = {}) }) {
    PlanScreen(
        PlanUiState.Ready(PlanSamples.page),
        PlanActions(RowActions({}, {}, {}, {}), {}, { _, _, _ -> }, {}, {}),
    )
}

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

/** The Log work sheet's content on the sheet colour (the modal sheet itself is a separate window). */
@Composable
private fun LogWorkSheet(logWork: LogWorkState, picker: TaskPickerState? = null) =
    Box(Modifier.fillMaxSize().background(QuestTheme.colors.card).padding(top = 24.dp)) { LogWorkSheetContent(LogWorkSheetState(logWork, picker), LogWorkActions.None) }

private val logWorkForm = LogWorkForm.of(LogWorkSamples.draft)

/** Every component and screen with sample data; each is captured light/dark × phone/tablet. */
val scenes = listOf(
    Scene("task-card", SceneKind.Component) {
        (Samples.home.questWork + Samples.home.other).forEach { TaskCard(it, noCardActions) }
        TaskCard(Samples.home.questWork.first(), noCardActions, Modifier.width(260.dp))
    },
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
    Scene("focus-running", SceneKind.Screen) { Focus(FocusUiState.Session(Samples.focusSession, Samples.focusSession.title, Samples.focusSession.description, Samples.home.quests)) },
    Scene("focus-paused", SceneKind.Screen) {
        val paused = Samples.pausedSession.copy(quest = null, header = "FOCUS", timer = "36:00", description = "")
        Focus(FocusUiState.Session(paused, paused.title, paused.description, Samples.home.quests))
    },
    Scene("focus-pick", SceneKind.Screen) { Focus(FocusUiState.Pick(Samples.pick.tasks)) },
    Scene("focus-pick-empty", SceneKind.Screen) { Focus(FocusUiState.Pick(emptyList())) },
    Scene("task-sheet", SceneKind.Screen) { Sheet(TaskSheetState.Open(Samples.detail, Samples.detail.title, Samples.detail.description, SheetPage.Details, null)) },
    Scene("task-sheet-overdue", SceneKind.Screen) {
        val overdue = Samples.detail.copy(dueOn = DayValue(iso = "2026-11-20", label = "Nov 20, 2026"), overdue = true)
        Sheet(TaskSheetState.Open(overdue, overdue.title, overdue.description, SheetPage.Details, null))
    },
    Scene("repeat-week", SceneKind.Screen) { Sheet(repeatState(RepeatForm.of(Samples.repeatDraft), Samples.repeatCheck)) },
    Scene("repeat-month-error", SceneKind.Screen) {
        val form = RepeatForm.of(Samples.repeatDraft).copy(every = "0", frequency = RepeatFrequency.MONTH, endsOnDate = true, ends = "2026-11-01")
        Sheet(repeatState(form, Samples.repeatCheck.copy(summary = "", error = "Enter an interval from 1 to 999.")), repeats = true)
    },
    Scene("plan", SceneKind.Screen) { Plan() },
    Scene("plan-not-planned", SceneKind.Screen, interact = {
        onNodeWithTag("plan-sections").performScrollToKey("not-planned")
        onNodeWithTag("not-planned").performClick()
        onNodeWithTag("plan-sections").performScrollToKey("unplanned-p5")
    }) { Plan() },
    Scene("plan-drag", SceneKind.Screen, interact = {
        onNodeWithTag("plan-week-${PlanSamples.thursday}").performClick()
        liftAndDrag("row:Water the plants", "row:Interval run")
    }) { Plan() },
    Scene("log-work", SceneKind.Screen) { LogWorkSheet(LogWorkState(logWorkForm, LogWorkSamples.check, null)) },
    Scene("log-work-new-task-error", SceneKind.Screen) {
        val form = logWorkForm.copy(newTitle = "Plan the workshop", questId = "q1", startMinute = 14 * 60, endMinute = 13 * 60 + 30)
        LogWorkSheet(LogWorkState(form, LogWorkSamples.newTaskError, null))
    },
    Scene("log-work-existing-task", SceneKind.Screen) {
        LogWorkSheet(LogWorkState(logWorkForm.copy(taskId = "t2"), LogWorkSamples.existingTask, null))
    },
    Scene("log-work-task-picker", SceneKind.Screen) {
        val picker = TaskPickerState("", LogWorkSamples.choices)
        LogWorkSheet(LogWorkState(logWorkForm, LogWorkSamples.check, null), picker)
    },
    Scene("log-work-task-create", SceneKind.Screen) {
        val picker = TaskPickerState("Plan the workshop", TaskChoices(emptyList(), "Create “Plan the workshop”"))
        LogWorkSheet(LogWorkState(logWorkForm, LogWorkSamples.check, null), picker)
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
) + shellScenes + questPickerScenes + questScenes + workspaceScenes + platformScenes + captureScenes + settingsScenes + accountScenes + taskFieldScenes
