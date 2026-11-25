package app.quest.android.core

import app.quest.android.capture.CaptureApplied
import app.quest.android.capture.CaptureDraft
import app.quest.android.capture.CaptureException
import app.quest.android.capture.CaptureView
import uniffi.quest_mobile.Account
import uniffi.quest_mobile.AiConnectionRow
import uniffi.quest_mobile.ArchiveData
import uniffi.quest_mobile.QuestDetail
import uniffi.quest_mobile.QuickAddForm
import uniffi.quest_mobile.SyncOutcome
import uniffi.quest_mobile.QuestHue
import uniffi.quest_mobile.QuestsData
import uniffi.quest_mobile.ReviewData
import uniffi.quest_mobile.SearchMode
import uniffi.quest_mobile.SearchOutcome
import uniffi.quest_mobile.SearchResult
import uniffi.quest_mobile.SearchToken
import uniffi.quest_mobile.TrashKind
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.FocusView
import uniffi.quest_mobile.HomeData
import uniffi.quest_mobile.HomeQuery
import uniffi.quest_mobile.LogWorkCheck
import uniffi.quest_mobile.LogWorkDraft
import uniffi.quest_mobile.Moment
import uniffi.quest_mobile.PlanSections
import uniffi.quest_mobile.AddPosition
import uniffi.quest_mobile.PlanTaskList
import uniffi.quest_mobile.RepeatCheck
import uniffi.quest_mobile.RepeatDraft
import uniffi.quest_mobile.RepeatWeekday
import uniffi.quest_mobile.RunningSession
import uniffi.quest_mobile.SettingsData
import uniffi.quest_mobile.SignInResult
import uniffi.quest_mobile.SyncState
import uniffi.quest_mobile.TaskChoices
import uniffi.quest_mobile.TaskDetail
import uniffi.quest_mobile.TaskSection
import uniffi.quest_mobile.WorkspacesData
import uniffi.quest_mobile.EntryCheck
import uniffi.quest_mobile.EntryDraft
import uniffi.quest_mobile.ListKind
import uniffi.quest_mobile.PlayerCard
import uniffi.quest_mobile.QuestNav
import uniffi.quest_mobile.TaskList
import uniffi.quest_mobile.TrackedData

/**
 * The Rust core as the app sees it. [RustQuestCore] is the only implementation that talks to
 * native code; tests use fakes, so ViewModels run without the `.so`.
 *
 * Every method only touches memory except opening (see [RustQuestCore.open]) and those documented as blocking
 * (signing in and out, account settings, network calls). Failures are
 * `uniffi.quest_mobile.MobileException`s with a message for the user.
 */
interface QuestCoreApi : WorkspacesApi {
    fun home(now: Moment, query: HomeQuery): HomeData

    /** Adds a task for today; the core picks its quest by the desktop rule. Returns the new id. */
    fun addTask(title: String, section: TaskSection, position: AddPosition, query: HomeQuery, now: Moment): String

    fun toggleTask(taskId: String, now: Moment)

    fun renameTask(taskId: String, title: String)

    fun startTimer(taskId: String, now: Moment)

    fun stopTimer(now: Moment)

    fun deleteTask(taskId: String, now: Moment)

    /** The undo toast's action, or (without a toast) restores the last deleted task. */
    fun undo(now: Moment)

    fun dismissToast(now: Moment)

    fun taskDetail(taskId: String, now: Moment): TaskDetail

    fun setTaskDescription(taskId: String, description: String)

    fun setTaskQuest(taskId: String, questId: String?)

    /** Plans the task for [plannedOn] (ISO, "2026-11-24"); null unplans it. A later deadline is pulled along. */
    fun setTaskPlanned(taskId: String, plannedOn: String?, now: Moment)

    fun repeatDraft(taskId: String, now: Moment): RepeatDraft

    fun setTaskRepeat(taskId: String, draft: RepeatDraft, now: Moment)

    fun clearTaskRepeat(taskId: String, now: Moment)

    /**
     * What Focus shows: the running task, else [focusTask] (started or picked last on this device;
     * unknown or done is ignored), else the open task worked on last; or today's tasks to pick from.
     */
    fun focus(now: Moment, focusTask: String?): FocusView

    /** The running timer for the ongoing notification; null when nothing runs. */
    fun runningSession(now: Moment): RunningSession?

    /** Whether the task is open (not done, not deleted) in any workspace; a paused notification stays while it is. */
    fun isTaskOpen(taskId: String): Boolean

    /** Focus's Complete: completes the task (stops its timer, leaves the undo toast); a done task is a no-op. */
    fun completeTask(taskId: String, now: Moment)

    fun planSections(now: Moment): PlanSections

    fun leavePlan()

    fun placeTask(taskId: String, list: PlanTaskList, beforeTaskId: String?, now: Moment)

    /** Tasks to pick for Log work, matching [query]. */
    fun taskChoices(query: String): TaskChoices

    /** Log work from the button: the quarter hour that just ended, today. */
    fun logWorkNow(now: Moment): LogWorkDraft

    fun checkLogWork(draft: LogWorkDraft, now: Moment): LogWorkCheck

    /** Saves the session; a wrong field throws `InvalidForm`. Returns the entry's id. */
    fun logWork(draft: LogWorkDraft, now: Moment): String

    /** One list (Today, Inbox, Logbook, a quest's); `UnknownQuest` when its quest is gone. */
    fun taskList(kind: ListKind, now: Moment): TaskList

    /** Adds a task with the list's defaults (Today: planned today; a quest's list: its quest). Returns its id. */
    fun addToList(kind: ListKind, title: String, now: Moment): String
    fun quickDayLabel(iso: String): String
    fun listQuickDefaults(kind: ListKind, now: Moment): uniffi.quest_mobile.QuickFields
    fun toggleListTask(id: String, now: Moment)
    fun setTaskDue(id: String, day: String?, now: Moment)
    fun previewQuickAdd(text: String, defaults: uniffi.quest_mobile.QuickFields, now: Moment): uniffi.quest_mobile.QuickPreview
    fun addQuickDraft(text: String, description: String, defaults: uniffi.quest_mobile.QuickFields, now: Moment): String
    fun quickAddQuestSuggestions(query: String, now: Moment): List<uniffi.quest_mobile.QuestChip>
    fun taskQuestOptions(query: String, now: Moment): List<uniffi.quest_mobile.QuestChip>
    fun taskLoggedEntries(id: String, now: Moment): List<uniffi.quest_mobile.LoggedEntry>

    /** The Quests tab: active and completed quests with this week's hours. */
    fun questNav(now: Moment): QuestNav

    /** The Tracked tab: today's timeline and this week. */
    fun tracked(now: Moment): TrackedData

    /** The mini player above the tabs. */
    fun player(now: Moment): PlayerCard

    /** The entry editor's fields as it opens; `UnknownEntry` once the session is gone. */
    fun entryDraft(entryId: String, now: Moment): EntryDraft

    fun checkEntry(draft: EntryDraft, now: Moment): EntryCheck

    /** Saves times and task; a wrong field throws `InvalidForm`. */
    fun saveEntry(draft: EntryDraft, now: Moment)

    /** Deletes the session softly; the toast offers Undo. */
    fun deleteEntry(entryId: String, now: Moment)

    /** The session keeps its times and becomes Organizing (its task is "Back to"). */
    fun bookEntryAsOrganizing(entryId: String)

    /** The Quests tab: the figures, active and completed cards, a new quest's colours. */
    fun quests(now: Moment): QuestsData

    /**
     * Creates a quest from the editor: trimmed title (`EmptyTitle` when blank) and [hue] (null: the
     * first unused colour). Returns its id.
     */
    fun createQuest(title: String, hue: QuestHue?): String

    /** One quest's page; `UnknownQuest` when it is gone (deleted, or never synced here). */
    fun questDetail(questId: String, now: Moment): QuestDetail

    /** An empty or unchanged title saves nothing. */
    fun renameQuest(questId: String, title: String)

    fun setQuestHue(questId: String, hue: QuestHue)

    /** Complete quest, Reopen quest or Unarchive, by the quest's status; the toast offers Undo. */
    fun toggleQuestStatus(questId: String, now: Moment)

    /** The archive's Unarchive: an archived quest becomes active (no toast, as on the desktop). */
    fun unarchiveQuest(questId: String)

    /** Soft-deletes the quest; the toast offers Undo. */
    fun deleteQuest(questId: String, now: Moment)

    /** The quest page's add field: today or the backlog, at [position]. Returns the new id. */
    fun addQuestTask(questId: String, title: String, backlog: Boolean, position: AddPosition, now: Moment): String

    /** The week with [week] in it (any ISO day; null: this week). */
    fun review(week: String?, now: Moment): ReviewData

    /** Deleted quests, tasks, checklist items and sessions, newest first. */
    fun archive(now: Moment): ArchiveData

    /** Brings a deleted thing back (and clears its delete toast); `Rejected` when it is no longer deleted. */
    fun restore(kind: TrashKind, id: String, now: Moment)

    /** The search's entries for [query] in [mode], ranked as the desktop palette. */
    fun search(mode: SearchMode, query: String, now: Moment): List<SearchResult>

    fun searchPlaceholder(mode: SearchMode): String

    /** Runs an entry's [token] (data actions happen here) and says what the app shows next. */
    fun runSearch(token: SearchToken, now: Moment): SearchOutcome

    fun syncState(): SyncState

    /** The signed-in account; null for the guest. */
    fun account(): Account?

    /**
     * Starts signing in at [serverUrl]: returns the page to open in a Custom Tab. It comes back to
     * `app.quest.android:/oauth2redirect`, or in release builds at the production server to its App Link
     * `https://…/auth/android` ([finishSignIn]). Memory-only; refuses disallowed URLs.
     */
    fun beginSignIn(serverUrl: String): String

    /**
     * The browser came back with [redirectUrl]: checks it, gets the tokens and switches to the account (or renews
     * an expired session). A return from signing out is `NotASignIn`. Blocks on the network: never on the main thread.
     */
    fun finishSignIn(redirectUrl: String): SignInResult

    /** "Import guest data?" answered Yes: moves it into the account. Blocks (up to about 20 s). Returns how many items moved. */
    fun importGuestData(): UInt

    /** The warning before signing out (changes not yet on the server); null when nothing is lost. Memory-only. */
    fun signOutWarning(): String?

    /**
     * Signs out (revokes the sign-in, deletes the account's tokens and local data) and switches to the guest. Returns
     * the page that ends the browser's sign-in too. Blocks (up to about 10 s): never on the main thread.
     */
    fun signOut(): String

    /** Turns the account's AI features on or off on the server. Blocks on the network: never on the main thread. */
    fun setAiEnabled(enabled: Boolean): Account

    fun settings(): SettingsData

    fun setWeeklyTarget(minutes: UInt)

    /** Runs due work (e.g. recurring tasks) and returns when the core wants the next tick (epoch ms). */
    fun tick(now: Moment): Long

    /** Start of the next local day (midnight, or the DST jump) plus slack, epoch ms; for the widget's day-change alarm. */
    fun nextDayMs(now: Moment): Long

    /**
     * Blocks until the outbox is empty and catch-up is done (Idle), sync cannot get on (Offline) or
     * [timeoutMs] passes. Never on the main thread; other core calls keep working meanwhile.
     */
    fun syncUntilIdle(timeoutMs: Long): SyncOutcome

    /** Blocks until local changes are in SQLite (true) or [timeoutMs] passes (false). Never on the main thread. */
    fun flush(timeoutMs: Long): Boolean

    /** The quick-add sheet (share, shortcut, tile): [sharedText] split into title and description, quest and day presets. */
    fun quickAddForm(sharedText: String?, now: Moment): QuickAddForm

    /** Adds a task at the end of [day] (ISO; null: backlog) with [questId] (null: no quest). Returns its id. */
    fun quickAdd(title: String, description: String, questId: String?, day: String?, now: Moment): String

    /**
     * Sends [text] through the server to the AI and returns its proposal. Blocks on the network (up to about
     * 75 s): never on the main thread. Failures are [CaptureException]s. [language] is a BCP 47 tag.
     */
    fun capture(text: String, language: String, now: Moment): CaptureDraft

    /** How the (edited) [draft] reads: labels, errors, quest options, the Create button. Pure. */
    fun captureView(draft: CaptureDraft, now: Moment): CaptureView

    /** Creates everything selected in [draft] as one change (all or nothing); returns the snackbar to show. */
    fun applyCapture(draft: CaptureDraft, now: Moment): CaptureApplied

    /** The snackbar's Undo after [applyCapture]: deletes what that capture created (once; else nothing). */
    fun undoCapture(now: Moment)

    /**
     * The AI clients connected to the account (MCP). Blocks on the network (up to about 20 s): never on the main
     * thread. Failures are [ConnectionsException]s.
     */
    fun aiConnections(now: Moment): List<AiConnectionRow>

    /** Revokes one connected AI client. Blocks on the network like [aiConnections]; failures likewise. */
    fun revokeAiConnection(id: String)

    /** The network is back: sync retries now instead of after its backoff. */
    fun reconnectNow()
}

/** The repeat dialog's rules (pure functions of the core): live check and weekday toggling. */
interface RepeatRules {
    fun check(draft: RepeatDraft, now: Moment): RepeatCheck

    fun toggleWeekday(draft: RepeatDraft, day: RepeatWeekday, now: Moment): RepeatDraft
}

/** Checks a server URL before signing in; returns the problem to show, or null when it is allowed. */
fun interface ServerUrlPolicy {
    fun problemWith(url: String): String?
}

/** Labels for days the user picks (pure core functions). */
fun interface DayLabels {
    /** [iso] with its label as the task sheet shows it; `InvalidDate` when it is not a day. */
    fun picked(iso: String): DayValue
}

/** The platform's clock and time zone. */
fun interface Clock {
    fun now(): Moment
}

/** The core's workspaces (part of [QuestCoreApi]; its own interface so fakes can share one). */
interface WorkspacesApi {
    /** The switcher and "Manage workspaces"; also notices a workspace deleted elsewhere. */
    fun workspaces(now: Moment): WorkspacesData

    /** Shows [workspaceId] from now on (remembered on this device); the core drops the old one's undo. */
    fun switchWorkspace(workspaceId: String)

    /** Creates a workspace and shows it; `InvalidForm` says what to fix in the name. Returns its id. */
    fun createWorkspace(name: String): String

    fun renameWorkspace(workspaceId: String, name: String)

    /** The delete confirmation's text, or `Rejected` for the last workspace. */
    fun deleteWorkspaceQuestion(workspaceId: String): String

    fun deleteWorkspace(workspaceId: String, now: Moment)

    /** Restores a deleted workspace; `InvalidForm` while a live one has its name. */
    fun restoreWorkspace(workspaceId: String)
}
