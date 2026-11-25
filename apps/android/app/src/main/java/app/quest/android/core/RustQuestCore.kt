package app.quest.android.core

import app.quest.android.BuildConfig
import app.quest.android.capture.CaptureApplied
import app.quest.android.capture.CaptureDraft
import app.quest.android.capture.CaptureView
import java.time.ZoneId
import uniffi.quest_mobile.Account
import uniffi.quest_mobile.AiConnectionRow
import uniffi.quest_mobile.ArchiveData
import uniffi.quest_mobile.QuestDetail
import uniffi.quest_mobile.QuickAddForm
import uniffi.quest_mobile.SyncOutcome
import uniffi.quest_mobile.UrlPolicy
import uniffi.quest_mobile.QuestHue
import uniffi.quest_mobile.QuestsData
import uniffi.quest_mobile.ReviewData
import uniffi.quest_mobile.SearchMode
import uniffi.quest_mobile.SearchOutcome
import uniffi.quest_mobile.SearchResult
import uniffi.quest_mobile.SearchToken
import uniffi.quest_mobile.TrashKind
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.ChangeListener
import uniffi.quest_mobile.FocusView
import uniffi.quest_mobile.HomeData
import uniffi.quest_mobile.HomeQuery
import uniffi.quest_mobile.LogWorkCheck
import uniffi.quest_mobile.LogWorkDraft
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.Moment
import uniffi.quest_mobile.MoveTarget
import uniffi.quest_mobile.PlanData
import uniffi.quest_mobile.PlanWindow
import uniffi.quest_mobile.QuestCore
import uniffi.quest_mobile.RepeatCheck
import uniffi.quest_mobile.RepeatDraft
import uniffi.quest_mobile.RepeatWeekday
import uniffi.quest_mobile.RunningSession
import uniffi.quest_mobile.SettingsData
import uniffi.quest_mobile.SignInResult
import uniffi.quest_mobile.TokenStore
import uniffi.quest_mobile.SyncState
import uniffi.quest_mobile.TaskChoices
import uniffi.quest_mobile.TaskDetail
import uniffi.quest_mobile.TaskSection
import uniffi.quest_mobile.WorkspacesData
import uniffi.quest_mobile.checkRepeat
import uniffi.quest_mobile.checkServerUrl
import uniffi.quest_mobile.pickedDay
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.planEdgeSpeed
import uniffi.quest_mobile.planWindow
import uniffi.quest_mobile.toggleRepeatWeekday

/** [QuestCoreApi] over the UniFFI bindings of `crates/quest-mobile`. */
class RustQuestCore private constructor(private val core: QuestCore) : QuestCoreApi {
    override fun home(now: Moment, query: HomeQuery): HomeData = core.home(now, query)

    override fun addTask(title: String, section: TaskSection, position: AddPosition, query: HomeQuery, now: Moment): String =
        core.addTask(title, section, position, query, now)

    override fun toggleTask(taskId: String, now: Moment) = core.toggleTask(taskId, now)

    override fun renameTask(taskId: String, title: String) = core.renameTask(taskId, title)

    override fun startTimer(taskId: String, now: Moment) = core.startTimer(taskId, now)

    override fun stopTimer(now: Moment) = core.stopTimer(now)

    override fun deleteTask(taskId: String, now: Moment) = core.deleteTask(taskId, now)

    override fun undo(now: Moment) = core.undo(now)

    override fun dismissToast(now: Moment) = core.dismissToast(now)

    override fun taskDetail(taskId: String, now: Moment): TaskDetail = core.taskDetail(taskId, now)

    override fun setTaskDescription(taskId: String, description: String) = core.setTaskDescription(taskId, description)

    override fun setTaskQuest(taskId: String, questId: String?) = core.setTaskQuest(taskId, questId)

    override fun setTaskPlanned(taskId: String, plannedOn: String?, now: Moment) = core.setTaskPlanned(taskId, plannedOn, now)

    override fun repeatDraft(taskId: String, now: Moment): RepeatDraft = core.repeatDraft(taskId, now)

    override fun setTaskRepeat(taskId: String, draft: RepeatDraft, now: Moment) = core.setTaskRepeat(taskId, draft, now)

    override fun clearTaskRepeat(taskId: String, now: Moment) = core.clearTaskRepeat(taskId, now)

    override fun focus(now: Moment, focusTask: String?): FocusView = core.focus(now, focusTask)

    override fun runningSession(now: Moment): RunningSession? = core.runningSession(now)

    override fun isTaskOpen(taskId: String): Boolean = core.isTaskOpen(taskId)

    override fun workspaces(now: Moment): WorkspacesData = core.workspaces(now)

    override fun switchWorkspace(workspaceId: String) = core.switchWorkspace(workspaceId)

    override fun createWorkspace(name: String): String = core.createWorkspace(name)

    override fun renameWorkspace(workspaceId: String, name: String) = core.renameWorkspace(workspaceId, name)

    override fun deleteWorkspaceQuestion(workspaceId: String): String = core.deleteWorkspaceQuestion(workspaceId)

    override fun deleteWorkspace(workspaceId: String, now: Moment) = core.deleteWorkspace(workspaceId, now)

    override fun restoreWorkspace(workspaceId: String) = core.restoreWorkspace(workspaceId)

    override fun completeTask(taskId: String, now: Moment) = core.completeTask(taskId, now)

    override fun plan(window: PlanWindow, questFilter: String?, now: Moment): PlanData = core.plan(window, questFilter, now)

    override fun leavePlan() = core.leavePlan()

    override fun planAddTask(day: String, title: String, position: AddPosition, now: Moment): String = core.planAddTask(day, title, position, now)

    override fun planMoveTask(taskId: String, day: String?, beforeTaskId: String?, now: Moment) = core.planMoveTask(taskId, day, beforeTaskId, now)

    override fun planNudgeTask(taskId: String, up: Boolean, now: Moment) = core.planNudgeTask(taskId, up, now)

    override fun planShiftTask(taskId: String, later: Boolean, now: Moment) = core.planShiftTask(taskId, later, now)

    override fun moveTargets(now: Moment): List<MoveTarget> = core.moveTargets(now)

    override fun taskChoices(query: String): TaskChoices = core.taskChoices(query)

    override fun logWorkNow(now: Moment): LogWorkDraft = core.logWorkNow(now)

    override fun checkLogWork(draft: LogWorkDraft, now: Moment): LogWorkCheck = core.checkLogWork(draft, now)

    override fun logWork(draft: LogWorkDraft, now: Moment): String = core.logWork(draft, now)

    override fun quests(now: Moment): QuestsData = core.quests(now)

    override fun createQuest(title: String, hue: QuestHue?): String = core.createQuest(title, hue)

    override fun questDetail(questId: String, now: Moment): QuestDetail = core.questDetail(questId, now)

    override fun renameQuest(questId: String, title: String) = core.renameQuest(questId, title)

    override fun setQuestHue(questId: String, hue: QuestHue) = core.setQuestHue(questId, hue)

    override fun toggleQuestStatus(questId: String, now: Moment) = core.toggleQuestStatus(questId, now)

    override fun unarchiveQuest(questId: String) = core.unarchiveQuest(questId)

    override fun deleteQuest(questId: String, now: Moment) = core.deleteQuest(questId, now)

    override fun addQuestTask(questId: String, title: String, backlog: Boolean, position: AddPosition, now: Moment): String =
        core.addQuestTask(questId, title, backlog, position, now)

    override fun review(week: String?, now: Moment): ReviewData = core.review(week, now)

    override fun archive(now: Moment): ArchiveData = core.archive(now)

    override fun restore(kind: TrashKind, id: String, now: Moment) = core.restore(kind, id, now)

    override fun search(mode: SearchMode, query: String, now: Moment): List<SearchResult> = core.search(mode, query, now)

    override fun searchPlaceholder(mode: SearchMode): String = core.searchPlaceholder(mode)

    override fun runSearch(token: SearchToken, now: Moment): SearchOutcome = core.runSearch(token, now)

    override fun syncState(): SyncState = core.syncState()

    override fun account(): Account? = core.account()

    // No App Link origin: every build signs in through the private-use scheme `app.quest.android:/oauth2redirect`.
    override fun beginSignIn(serverUrl: String): String = core.beginSignIn(serverUrl, BUILD_URL_POLICY, null)

    override fun finishSignIn(redirectUrl: String): SignInResult = core.finishSignIn(redirectUrl)

    override fun importGuestData(): UInt = core.importGuestData()

    override fun signOutWarning(): String? = core.signOutWarning()

    override fun signOut(): String = core.signOut()

    override fun setAiEnabled(enabled: Boolean): Account = core.setAiEnabled(enabled)

    override fun settings(): SettingsData = core.settings()

    override fun setWeeklyTarget(minutes: UInt) = core.setWeeklyTarget(minutes)

    override fun tick(now: Moment): Long = core.tick(now)

    override fun nextDayMs(now: Moment): Long = core.nextDayMs(now)

    override fun syncUntilIdle(timeoutMs: Long): SyncOutcome = core.syncUntilIdle(timeoutMs.toULong())

    override fun flush(timeoutMs: Long): Boolean = core.flush(timeoutMs.toULong())

    override fun quickAddForm(sharedText: String?, now: Moment): QuickAddForm = core.quickAddForm(sharedText, now)

    override fun quickAdd(title: String, description: String, questId: String?, day: String?, now: Moment): String =
        core.quickAdd(title, description, questId, day, now)

    override fun capture(text: String, language: String, now: Moment): CaptureDraft = captureCall { core.capture(text, language, now).toApp() }

    override fun captureView(draft: CaptureDraft, now: Moment): CaptureView = core.captureView(draft.toFfi(), now).toApp()

    override fun applyCapture(draft: CaptureDraft, now: Moment): CaptureApplied =
        core.applyCapture(draft.toFfi(), now).let { CaptureApplied(it.message, it.undoLabel, it.showForMs.toLong()) }

    override fun undoCapture(now: Moment) = core.undoCapture(now)

    override fun aiConnections(now: Moment): List<AiConnectionRow> = connectionsCall { core.aiConnections(now) }

    override fun revokeAiConnection(id: String) = connectionsCall { core.revokeAiConnection(id) }

    override fun reconnectNow() = core.reconnectNow()

    companion object {
        /**
         * Opens the active profile in [dataDir] (the signed-in account, else the guest). Disk IO: call it on
         * `Dispatchers.IO`. [tokens] keeps the accounts' tokens. [onChange] runs on the core's worker thread
         * whenever its data or sync state changed, and after switching accounts.
         */
        fun open(dataDir: String, tokens: TokenStore, onChange: () -> Unit): RustQuestCore {
            val listener = object : ChangeListener {
                override fun onChange() = onChange()
            }
            return RustQuestCore(QuestCore.open(dataDir, tokens, listener))
        }
    }
}

/** The core's repeat dialog rules; pure, safe on the main thread. */
object RustRepeatRules : RepeatRules {
    override fun check(draft: RepeatDraft, now: Moment): RepeatCheck = checkRepeat(draft, now)

    override fun toggleWeekday(draft: RepeatDraft, day: RepeatWeekday, now: Moment): RepeatDraft = toggleRepeatWeekday(draft, day, now)
}

/** The core's plan rules; pure, safe on the main thread (also per frame while dragging). */
object RustPlanRules : PlanRules {
    override fun window(firstVisible: String?, visibleDays: Int, built: PlanWindow?, now: Moment): PlanWindow =
        planWindow(firstVisible, visibleDays.coerceAtLeast(1).toUInt(), built, now)

    override fun edgeSpeed(pos: Float, start: Float, end: Float): Float = planEdgeSpeed(pos, start, end)
}

/**
 * Plain http to the emulator's host (10.0.2.2) only in debug builds: on a phone that address is just
 * some machine in the local network. Loopback (e.g. through `adb reverse`) is allowed in both.
 */
private val BUILD_URL_POLICY = if (BuildConfig.DEBUG) UrlPolicy.DEVELOPMENT else UrlPolicy.RELEASE

/** The core's URL rule (https always, http only for loopback and, in debug builds, the emulator's host). */
object RustServerUrlPolicy : ServerUrlPolicy {
    override fun problemWith(url: String): String? =
        try {
            checkServerUrl(url, BUILD_URL_POLICY)
            null
        } catch (e: MobileException) {
            e.message ?: "This server URL is not allowed. Use an https:// address."
        }
}

/** The core's label for a day picked in the date picker; pure, safe on the main thread. */
object RustDayLabels : DayLabels {
    override fun picked(iso: String): DayValue = pickedDay(iso)
}

object SystemClock : Clock {
    override fun now() = Moment(System.currentTimeMillis(), ZoneId.systemDefault().id)
}

/** Runs a connections call; the core's errors become [ConnectionsException]s (offline when the server was unreachable). */
private inline fun <T> connectionsCall(call: () -> T): T = try {
    call()
} catch (e: MobileException) {
    throw ConnectionsException(e.message.orEmpty(), offline = e is MobileException.ServerUnreachable, gone = e is MobileException.UnknownConnection)
}
