package app.quest.android

import app.quest.android.capture.CaptureApplied
import app.quest.android.capture.CaptureDraft
import app.quest.android.capture.CaptureException
import app.quest.android.capture.CaptureFailure
import app.quest.android.capture.CaptureView
import app.quest.android.settings.CapturePreferences
import app.quest.android.settings.CapturePrefs
import kotlinx.coroutines.flow.combine
import app.quest.android.core.Clock
import app.quest.android.core.QuestCoreApi
import app.quest.android.core.WorkspacesApi
import app.quest.android.core.RepeatRules
import app.quest.android.settings.FocusPreferences
import app.quest.android.settings.FocusPrefs
import app.quest.android.settings.HomePreferences
import app.quest.android.settings.HomePrefs
import kotlinx.coroutines.flow.MutableStateFlow
import app.quest.android.core.ConnectionsException
import uniffi.quest_mobile.Account
import uniffi.quest_mobile.AiConnectionRow
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.LogWorkCheck
import uniffi.quest_mobile.LogWorkDraft
import uniffi.quest_mobile.TaskChoices
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.FocusPickTask
import uniffi.quest_mobile.FocusSession
import uniffi.quest_mobile.FocusView
import uniffi.quest_mobile.HomeData
import uniffi.quest_mobile.HomeQuery
import uniffi.quest_mobile.HomeTask
import uniffi.quest_mobile.LoggedEntry
import uniffi.quest_mobile.MobileException
import uniffi.quest_mobile.Moment
import uniffi.quest_mobile.MoveTarget
import uniffi.quest_mobile.PlanData
import uniffi.quest_mobile.PlanWindow
import uniffi.quest_mobile.QuestChip
import uniffi.quest_mobile.RepeatCheck
import uniffi.quest_mobile.RepeatDraft
import uniffi.quest_mobile.RepeatFrequency
import uniffi.quest_mobile.RepeatPinChoice
import uniffi.quest_mobile.RepeatWeekday
import uniffi.quest_mobile.PostponedCount
import uniffi.quest_mobile.RunningSession
import uniffi.quest_mobile.SessionKind
import uniffi.quest_mobile.SettingsData
import uniffi.quest_mobile.SignInResult
import uniffi.quest_mobile.SyncState
import uniffi.quest_mobile.TaskDetail
import uniffi.quest_mobile.TaskSection
import uniffi.quest_mobile.UndoToast
import uniffi.quest_mobile.ArchiveData
import uniffi.quest_mobile.QuestCard
import uniffi.quest_mobile.QuestDetail
import uniffi.quest_mobile.QuestHue
import uniffi.quest_mobile.QuestsData
import uniffi.quest_mobile.ReviewData
import uniffi.quest_mobile.SearchMode
import uniffi.quest_mobile.SearchOutcome
import uniffi.quest_mobile.SearchResult
import uniffi.quest_mobile.SearchToken
import uniffi.quest_mobile.TrashKind
import uniffi.quest_mobile.QuickAddForm
import uniffi.quest_mobile.SyncOutcome

/** Monday 2026-11-23 09:30 in Berlin. */
const val NOW_MS = 1_795_422_600_000L
const val ZONE = "Europe/Berlin"

/** A clock the test moves by hand. */
class FakeClock(var epochMs: Long = NOW_MS) : Clock {
    override fun now() = Moment(epochMs, ZONE)
}

object Samples {
    val notPostponed = PostponedCount(0u, 0u, 0u)

    val writing = QuestChip("q1", "Write the book", "#5c8fd9")
    val fitness = QuestChip("q2", "Fitness", "#62b87a")

    private fun task(id: String, title: String, quest: QuestChip?, done: Boolean = false, tracked: String? = null, running: Boolean = false) =
        HomeTask(id = id, title = title, done = done, quest = quest, tracked = tracked, timerRunning = running, canStartTimer = !done, plannedOn = null, dueOn = null, overdue = false, postponed = notPostponed)

    val home = HomeData(
        dayLabel = "Monday, November 23",
        dayIso = "2026-11-23",
        todayHours = "1.5h",
        weekHours = "6.2h",
        weeklyTarget = "20h",
        weekProgress = 0.31f,
        questWork = listOf(
            task("t1", "Outline chapter 3", writing, tracked = "1:12:05", running = true),
            task("t4", "Edit chapter 2", writing, tracked = "36:00"),
            task("t2", "Morning run", fitness, done = true, tracked = "45:00"),
        ),
        other = listOf(task("t3", "Call the bank", null)),
        questFilter = null,
        addQuest = writing,
        quests = listOf(writing, fitness),
        toast = null,
    )

    val quickAddForm = QuickAddForm(
        title = "",
        description = "",
        quest = writing,
        questOptions = listOf(writing, fitness),
        today = DayValue("2026-11-23", "Mon, Nov 23"),
    )

    val emptyHome = home.copy(questWork = emptyList(), other = emptyList(), todayHours = "0h", weekHours = "0h", weekProgress = 0f)

    val toast = UndoToast(text = "Deleted “Call the bank”", actionLabel = "Undo", expiresAtMs = NOW_MS + 6_000)

    val detail = TaskDetail(
        id = "t1",
        title = "Outline chapter 3",
        description = "Three scenes; end on the storm.",
        done = false,
        quest = writing,
        questOptions = listOf(writing, fitness),
        plannedOn = DayValue(iso = "2026-11-23", label = "Nov 23, 2026"),
        dueOn = null,
        overdue = false,
        postponed = notPostponed,
        checklist = emptyList(),
        repeatLabel = "Does not repeat",
        repeats = false,
        loggedTotal = "1h 32m total",
        logged = listOf(
            LoggedEntry("e2", "Nov 23 · 08:18–now", "1h 12m", true),
            LoggedEntry("e0", "Nov 22 · 16:00–16:20", "20m", false),
        ),
    )

    val repeatDraft = RepeatDraft(
        every = "2",
        frequency = RepeatFrequency.WEEK,
        pin = RepeatPinChoice.NONE,
        weekdays = listOf(RepeatWeekday.MON, RepeatWeekday.THU),
        lastDay = false,
        monthDay = "",
        starts = "2026-11-23",
        endsOnDate = false,
        ends = "",
        timeZone = ZONE,
    )

    val repeatCheck = RepeatCheck(
        shownWeekdays = listOf(RepeatWeekday.MON, RepeatWeekday.THU),
        monthDayPlaceholder = "23",
        summary = "Every 2 weeks",
        error = null,
        starts = DayValue(iso = "2026-11-23", label = "Nov 23, 2026"),
        ends = null,
    )

    val focusSession = FocusSession(
        taskId = "t1",
        title = "Outline chapter 3",
        description = "Three scenes; end on the storm.",
        quest = writing,
        header = "FOCUS · WRITE THE BOOK",
        timer = "1:12:05",
        running = true,
        stateLabel = "RUNNING",
    )

    val pausedSession = focusSession.copy(running = false, stateLabel = "PAUSED")

    val pick = FocusView.Pick(
        listOf(
            FocusPickTask("t1", "Outline chapter 3", writing),
            FocusPickTask("t4", "Edit chapter 2", writing),
            FocusPickTask("t3", "Call the bank", null),
        ),
    )

    /** Started 1:12:05 before [NOW_MS]. */
    val running = RunningSession(SessionKind.TASK, "t1", "Outline chapter 3", writing.name, writing.colorHex, NOW_MS - 4_325_000, null, null, null)

    /** Organizing for 10 minutes, coming from "Outline chapter 3". */
    val organizing = RunningSession(SessionKind.ORGANIZING, null, "Organizing", null, null, NOW_MS - 600_000, "t1", "Outline chapter 3", null)

    val settings = SettingsData(weeklyTargetMinutes = 20u * 60u, weeklyTargetLabel = "20h")

    val account = Account(
        displayName = "ada@example.com",
        email = "ada@example.com",
        name = "Ada",
        serverUrl = "https://quest.example.com",
        aiEnabled = false,
        aiAvailable = true,
        aiFeatures = false,
    )

    val connections = listOf(
        AiConnectionRow("c1", "Claude Code", "localhost", "Connected Nov 3 · Last used today", "Read and write"),
        AiConnectionRow("c2", "Claude", "claude.ai", "Connected Oct 28 · Not used yet", "Read only"),
    )
}

/**
 * An in-memory [QuestCoreApi]: records calls, returns canned data, and fails where told to.
 * [nextWakeInMs] is what [tick] asks for, relative to the tick's "now".
 */
class FakeCore(
    var home: HomeData = Samples.home,
    var settings: SettingsData = Samples.settings,
    var account: Account? = null,
    var sync: SyncState = SyncState.SignedOut,
    var nextWakeInMs: Long = 60_000,
    /** Task details by id; a missing id is an unknown task. */
    val details: MutableMap<String, TaskDetail> = mutableMapOf("t1" to Samples.detail),
    var focus: FocusView = FocusView.Session(Samples.focusSession),
    var running: RunningSession? = null,
    var plan: PlanData = PlanSamples.plan,
    var logWorkCheck: LogWorkCheck = LogWorkSamples.check,
    var quests: QuestsData = QuestSamples.quests,
    /** Quest pages by id; a missing id is an unknown quest. */
    val questDetails: MutableMap<String, QuestDetail> = mutableMapOf("q1" to QuestSamples.detail),
    var review: ReviewData = QuestSamples.review,
    var archive: ArchiveData = QuestSamples.archive,
    /** What [search] lists, by mode and query. */
    var searchResults: (SearchMode, String) -> List<SearchResult> = { _, query -> QuestSamples.commands.filter { it.title.contains(query, ignoreCase = true) } },
    /** What [runSearch] returns, by token. */
    var searchOutcome: (SearchToken) -> SearchOutcome = { SearchOutcome.Done },
    /** What [syncUntilIdle] returns. */
    var syncOutcome: SyncOutcome = SyncOutcome.IDLE,
    /** Runs inside every [syncUntilIdle] call (e.g. to stop the caller meanwhile). */
    var onSync: () -> Unit = {},
    /** What [flush] returns. */
    var flushed: Boolean = true,
    var quickAddForm: QuickAddForm = Samples.quickAddForm,
    /** What [capture] returns, unless [captureFailure] is set. */
    var captureDraft: CaptureDraft = CaptureSamples.draft,
    var captureFailure: CaptureFailure? = null,
    /** What [captureView] answers for a draft. */
    var captureViewOf: (CaptureDraft) -> CaptureView = CaptureSamples::view,
    /** What [aiConnections] lists, unless [connectionsFailure] is set (also for [revokeAiConnection]). */
    var connections: List<AiConnectionRow> = Samples.connections,
    var connectionsFailure: ConnectionsException? = null,
    /** Runs inside every [aiConnections] call (e.g. to see the screen meanwhile). */
    var onConnections: () -> Unit = {},
    /** Runs inside every [revokeAiConnection] call. */
    var onRevoke: () -> Unit = {},
    val workspaces: FakeWorkspaces = FakeWorkspaces(),
) : QuestCoreApi, WorkspacesApi by workspaces {
    /** Every [capture] as text and language; every applied draft; [reconnectNow] calls. */
    val captures = mutableListOf<Pair<String, String>>()
    val appliedDrafts = mutableListOf<CaptureDraft>()
    var reconnects = 0

    /** The shared text of every [quickAddForm] read. */
    val quickAddReads = mutableListOf<String?>()

    /** Every [syncUntilIdle] and [flush] timeout. */
    val syncWaits = mutableListOf<Long>()
    val flushes = mutableListOf<Long>()

    /** The week of every [review] read. */
    val reviewWeeks = mutableListOf<String?>()

    /** Every [search] as mode and query. */
    val searches = mutableListOf<Pair<SearchMode, String>>()

    /** Every draft checked by [checkLogWork]. */
    val logWorkChecks = mutableListOf<LogWorkDraft>()
    val choiceQueries = mutableListOf<String>()

    /** The window and filter of every [plan] read. */
    val planReads = mutableListOf<Pair<PlanWindow, String?>>()

    val ticks = mutableListOf<Long>()
    val toggled = mutableListOf<String>()

    /** Every other action as "name arg…", in order. */
    val calls = mutableListOf<String>()
    val queries = mutableListOf<HomeQuery>()
    val signIns = mutableListOf<String>()

    /** The holder's change signal, as the Rust core's `ChangeListener`; set by [openedHolder][app.quest.android.ui.openedHolder]. */
    var onChange: () -> Unit = {}
    var failNext: Exception? = null
    var homeReads = 0

    /** The `focusTask` of every [focus] read. */
    val focusTasks = mutableListOf<String?>()

    private fun maybeFail() {
        failNext?.let {
            failNext = null
            throw it
        }
    }

    private fun record(call: String) {
        maybeFail()
        calls += call
    }

    override fun home(now: Moment, query: HomeQuery): HomeData {
        homeReads++
        queries += query
        return home
    }

    override fun addTask(title: String, section: TaskSection, position: AddPosition, query: HomeQuery, now: Moment): String {
        if (title.isBlank()) throw MobileException.EmptyTitle("Enter a title.")
        record("add $title $section $position ${query.questFilter} ${query.questPick}")
        return "new"
    }

    override fun toggleTask(taskId: String, now: Moment) {
        maybeFail()
        toggled += taskId
    }

    override fun renameTask(taskId: String, title: String) = record("rename $taskId $title")

    override fun startTimer(taskId: String, now: Moment) {
        record("start $taskId")
        running = Samples.running.copy(taskId = taskId)
    }

    override fun stopTimer(now: Moment) {
        record("stop")
        running = null
    }

    override fun deleteTask(taskId: String, now: Moment) {
        record("delete $taskId")
        details.remove(taskId)
    }

    override fun undo(now: Moment) {
        record("undo")
        val (oldQuests, oldDetails) = beforeStatusChange ?: return
        beforeStatusChange = null
        quests = oldQuests
        questDetails.clear()
        questDetails.putAll(oldDetails)
    }

    override fun dismissToast(now: Moment) = record("dismiss")

    override fun taskDetail(taskId: String, now: Moment): TaskDetail {
        maybeFail()
        return details[taskId] ?: throw MobileException.UnknownTask("This task is gone.")
    }

    override fun setTaskDescription(taskId: String, description: String) = record("describe $taskId $description")

    override fun setTaskQuest(taskId: String, questId: String?) = record("quest $taskId $questId")

    override fun setTaskPlanned(taskId: String, plannedOn: String?, now: Moment) = record("day $taskId $plannedOn")

    override fun repeatDraft(taskId: String, now: Moment): RepeatDraft {
        maybeFail()
        return Samples.repeatDraft
    }

    override fun setTaskRepeat(taskId: String, draft: RepeatDraft, now: Moment) =
        record("repeat $taskId ${draft.every} ${draft.frequency}")

    override fun clearTaskRepeat(taskId: String, now: Moment) = record("norepeat $taskId")

    override fun focus(now: Moment, focusTask: String?): FocusView {
        maybeFail()
        focusTasks += focusTask
        return focus
    }

    override fun runningSession(now: Moment): RunningSession? {
        maybeFail()
        return running
    }

    /** Tasks done or deleted (elsewhere); every other task is open. */
    val closedTasks = mutableSetOf<String>()

    override fun isTaskOpen(taskId: String): Boolean {
        maybeFail()
        return taskId !in closedTasks
    }

    override fun completeTask(taskId: String, now: Moment) {
        record("complete $taskId")
        if (running?.taskId == taskId) running = null
    }

    override fun plan(window: PlanWindow, questFilter: String?, now: Moment): PlanData {
        maybeFail()
        planReads += window to questFilter
        return plan
    }

    override fun leavePlan() = record("leave-plan")

    override fun planAddTask(day: String, title: String, position: AddPosition, now: Moment): String {
        if (title.isBlank()) throw MobileException.EmptyTitle("Enter a title.")
        record("plan-add $day $title $position")
        return "new"
    }

    override fun planMoveTask(taskId: String, day: String?, beforeTaskId: String?, now: Moment) = record("plan-move $taskId $day $beforeTaskId")

    override fun planNudgeTask(taskId: String, up: Boolean, now: Moment) = record("plan-nudge $taskId ${if (up) "up" else "down"}")

    override fun planShiftTask(taskId: String, later: Boolean, now: Moment) = record("plan-shift $taskId ${if (later) "later" else "earlier"}")

    override fun moveTargets(now: Moment): List<MoveTarget> {
        maybeFail()
        return PlanSamples.targets
    }

    override fun taskChoices(query: String): TaskChoices {
        maybeFail()
        choiceQueries += query
        return if (query.isEmpty()) LogWorkSamples.choices else TaskChoices(emptyList(), "Create “$query”")
    }

    override fun logWorkNow(now: Moment): LogWorkDraft {
        maybeFail()
        return LogWorkSamples.draft
    }

    override fun checkLogWork(draft: LogWorkDraft, now: Moment): LogWorkCheck {
        maybeFail()
        logWorkChecks += draft
        return logWorkCheck
    }

    override fun logWork(draft: LogWorkDraft, now: Moment): String {
        if (draft.taskId == null && draft.newTitle.isBlank()) throw MobileException.InvalidForm("Choose a task or type a title.")
        record("log ${draft.taskId} ${draft.newTitle} ${draft.questId} ${draft.day} ${draft.startMinute}-${draft.endMinute}")
        return "new-entry"
    }

    override fun quests(now: Moment): QuestsData {
        maybeFail()
        return quests
    }

    override fun createQuest(title: String, hue: QuestHue?): String {
        if (title.isBlank()) throw MobileException.EmptyTitle("Enter a name.")
        record("create-quest $title|$hue")
        return "q-new"
    }

    /** Fails every [questDetail] read while set. */
    var questDetailFailure: Exception? = null

    override fun questDetail(questId: String, now: Moment): QuestDetail {
        maybeFail()
        questDetailFailure?.let { throw it }
        return questDetails[questId] ?: throw MobileException.UnknownQuest("This quest is gone.")
    }

    override fun renameQuest(questId: String, title: String) {
        if (title.isBlank()) throw MobileException.EmptyTitle("Enter a name.")
        record("rename-quest $questId $title")
    }

    override fun setQuestHue(questId: String, hue: QuestHue) = record("hue $questId $hue")

    /** Quests and pages before the last status change, for [undo]. */
    private var beforeStatusChange: Pair<QuestsData, Map<String, QuestDetail>>? = null

    /** Moves the quest between Active and Completed as the core does (hours kept, toast with Undo). */
    override fun toggleQuestStatus(questId: String, now: Moment) {
        record("toggle-quest $questId")
        beforeStatusChange = quests to questDetails.toMap()
        quests = QuestSamples.toggled(quests, questId)
        questDetails[questId]?.let { questDetails[questId] = QuestSamples.toggled(it) }
    }

    /** The archived quest leaves the archive and is active again. */
    override fun unarchiveQuest(questId: String) {
        record("unarchive-quest $questId")
        val item = archive.archivedQuests.find { it.id == questId } ?: return
        archive = archive.copy(archivedQuests = archive.archivedQuests - item)
        val card = QuestCard(item.id, item.title, item.colorHex.orEmpty(), "0h", "0h total", "0 open tasks", "${item.title}, 0h this week, 0h total, 0 open tasks")
        quests = QuestSamples.listed(quests, quests.active + card, quests.completed)
    }

    override fun deleteQuest(questId: String, now: Moment) {
        record("delete-quest $questId")
        questDetails.remove(questId)
    }

    override fun addQuestTask(questId: String, title: String, backlog: Boolean, position: AddPosition, now: Moment): String {
        if (title.isBlank()) throw MobileException.EmptyTitle("Enter a title.")
        record("quest-add $questId $title ${if (backlog) "backlog" else "today"} $position")
        return "new"
    }

    override fun review(week: String?, now: Moment): ReviewData {
        maybeFail()
        reviewWeeks += week
        return review
    }

    override fun archive(now: Moment): ArchiveData {
        maybeFail()
        return archive
    }

    override fun restore(kind: TrashKind, id: String, now: Moment) = record("restore $kind $id")

    override fun search(mode: SearchMode, query: String, now: Moment): List<SearchResult> {
        maybeFail()
        searches += mode to query
        return searchResults(mode, query)
    }

    override fun searchPlaceholder(mode: SearchMode): String = if (mode == SearchMode.Commands) "Search or jump…" else "Choose…"

    override fun runSearch(token: SearchToken, now: Moment): SearchOutcome {
        record("run $token")
        return searchOutcome(token)
    }

    override fun syncState() = sync

    override fun account() = account

    override fun beginSignIn(serverUrl: String): String {
        record("begin sign in $serverUrl")
        signIns += serverUrl
        return "$serverUrl/auth/authorize?client=android"
    }

    /** What [finishSignIn] answers; signing in switches to [Samples.account] at the last begun server. */
    var signInResult: (redirect: String) -> SignInResult = {
        account = Samples.account.copy(serverUrl = signIns.lastOrNull() ?: Samples.account.serverUrl)
        sync = SyncState.Connecting
        SignInResult.SignedIn(offerGuestImport = false, account = account!!)
    }

    override fun finishSignIn(redirectUrl: String): SignInResult {
        record("finish sign in $redirectUrl")
        return signInResult(redirectUrl).also { onChange() }
    }

    var imported: UInt = 0u

    override fun importGuestData(): UInt {
        record("import guest data")
        onChange()
        return imported
    }

    var signOutWarning: String? = null

    override fun signOutWarning() = signOutWarning

    override fun signOut(): String {
        record("sign out")
        val server = account?.serverUrl ?: "https://quest.example.com"
        account = null
        sync = SyncState.SignedOut
        onChange()
        return "$server/auth/logout?return_to=app"
    }

    override fun setAiEnabled(enabled: Boolean): Account {
        record("ai enabled $enabled")
        val current = account ?: Samples.account
        return current.copy(aiEnabled = enabled, aiFeatures = enabled && current.aiAvailable).also {
            account = it
            onChange()
        }
    }

    override fun settings() = settings

    override fun setWeeklyTarget(minutes: UInt) {
        maybeFail()
        settings = SettingsData(minutes, "${minutes / 60u}h")
    }

    override fun tick(now: Moment): Long {
        maybeFail()
        ticks += now.epochMs
        return now.epochMs + nextWakeInMs
    }

    /** Midnight after [NOW_MS] in Berlin, plus the core's 5 ms slack. */
    override fun nextDayMs(now: Moment): Long = NEXT_DAY_MS

    override fun syncUntilIdle(timeoutMs: Long): SyncOutcome {
        syncWaits += timeoutMs
        onSync()
        return syncOutcome
    }

    override fun flush(timeoutMs: Long): Boolean {
        maybeFail()
        flushes += timeoutMs
        return flushed
    }

    override fun quickAddForm(sharedText: String?, now: Moment): QuickAddForm {
        maybeFail()
        quickAddReads += sharedText
        return quickAddForm
    }

    override fun quickAdd(title: String, description: String, questId: String?, day: String?, now: Moment): String {
        maybeFail()
        if (title.isBlank()) throw MobileException.EmptyTitle("Enter a title.")
        calls += "quick-add $title|$description|$questId|$day"
        return "new"
    }

    override fun capture(text: String, language: String, now: Moment): CaptureDraft {
        maybeFail()
        captures += text to language
        captureFailure?.let { throw CaptureException(it) }
        return captureDraft
    }

    override fun captureView(draft: CaptureDraft, now: Moment): CaptureView {
        maybeFail()
        return captureViewOf(draft)
    }

    override fun applyCapture(draft: CaptureDraft, now: Moment): CaptureApplied {
        maybeFail()
        appliedDrafts += draft
        return CaptureApplied("Created 1 quest, 1 task and 1 session", "Undo", 6_000)
    }

    override fun undoCapture(now: Moment) = record("undo-capture")

    override fun aiConnections(now: Moment): List<AiConnectionRow> {
        maybeFail()
        calls += "ai-connections"
        onConnections()
        connectionsFailure?.let { throw it }
        return connections
    }

    override fun revokeAiConnection(id: String) {
        record("revoke $id")
        onRevoke()
        connectionsFailure?.let { throw it }
        connections = connections.filterNot { it.id == id }
    }

    override fun reconnectNow() {
        reconnects++
    }
}

/** 2026-11-24 00:00 in Berlin + 5 ms. */
const val NEXT_DAY_MS = 1_795_474_800_005L

/** Repeat rules that echo: the check reports [error]; toggling adds or removes the day. */
class FakeRepeatRules(var error: String? = null) : RepeatRules {
    override fun check(draft: RepeatDraft, now: Moment) = Samples.repeatCheck.copy(shownWeekdays = draft.weekdays, error = error)

    override fun toggleWeekday(draft: RepeatDraft, day: RepeatWeekday, now: Moment): RepeatDraft =
        draft.copy(weekdays = if (day in draft.weekdays) draft.weekdays - day else draft.weekdays + day)
}

/** [HomePreferences] in memory. */
class FakeHomePreferences(initial: HomePrefs = HomePrefs()) : HomePreferences {
    override val prefs = MutableStateFlow(initial)

    override suspend fun setShowOther(show: Boolean) {
        prefs.value = prefs.value.copy(showOther = show)
    }

    override suspend fun setAddPosition(position: AddPosition) {
        prefs.value = prefs.value.copy(addPosition = position)
    }
}

/** [FocusPreferences] in memory. */
class FakeFocusPreferences(initial: FocusPrefs = FocusPrefs()) : FocusPreferences {
    override val prefs = MutableStateFlow(initial)

    override suspend fun setKeepScreenOn(keep: Boolean) {
        prefs.value = prefs.value.copy(keepScreenOn = keep)
    }

    override suspend fun setNotificationsAsked() {
        prefs.value = prefs.value.copy(notificationsAsked = true)
    }
}

class FakeCapturePreferences(initial: CapturePrefs = CapturePrefs()) : CapturePreferences {
    override val prefs = MutableStateFlow(initial)

    override suspend fun setPrivacyAcknowledged() {
        prefs.value = prefs.value.copy(privacyAcknowledged = true)
    }
}
