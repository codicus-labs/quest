//! The tools: each reads the input, decides with the pure functions in `read`/`write` against the
//! account's state, runs the commands through [`account::change`] and describes the result.

use axum::http::request::Parts;
use chrono::Utc;
use quest_domain::{EntityId, EntityKind, Snapshot};
use rmcp::handler::server::common::Extension;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{tool, tool_router};

use super::account::Scope;
use super::error::ToolError;
use super::types::*;
use super::view::{
    Clock, archived_out, entry_out, find_task, milestone_out, quest_out, task_out, time_zone, workspace_out,
    workspace_ref,
};
use super::{QuestMcp, account, capture, read, write};
use crate::oauth::McpPrincipal;

/// Who calls and when, in which zone and workspace.
struct Call {
    user: quest_protocol::UserId,
    clock: Clock,
    /// The `workspace` argument as given; resolved against the account's state when it is read.
    workspace: Option<String>,
}

/// Tool signatures spell out `Result<Json<T>, ToolError>`: the `tool` macro finds the output
/// schema in that exact shape, not through an alias.
type Out<T> = Result<Json<T>, ToolError>;

impl QuestMcp {
    fn call(&self, parts: &Parts, context: &CallContext) -> Result<Call, ToolError> {
        let principal = parts
            .extensions
            .get::<McpPrincipal>()
            .ok_or_else(|| ToolError::internal("reading the caller", "no principal on the request"))?;
        let zone = time_zone(context.time_zone.as_deref(), self.default_time_zone)?;
        Ok(Call { user: principal.user, clock: Clock { now: Utc::now(), zone }, workspace: context.workspace.clone() })
    }

    async fn snapshot(&self, call: &Call) -> Result<Snapshot, ToolError> {
        account::snapshot(&self.state, call.user, Scope::Named(call.workspace.as_deref())).await
    }

    async fn change<T>(
        &self,
        call: &Call,
        decide: impl FnOnce(&Snapshot) -> Result<(Vec<quest_domain::Command>, T), ToolError>,
    ) -> Result<(Snapshot, T), ToolError> {
        account::change(&self.state, call.user, Scope::Named(call.workspace.as_deref()), call.clock.now, decide).await
    }
}

fn task_result(snapshot: &Snapshot, id: EntityId, clock: &Clock) -> Out<TaskResult> {
    let (task, planned) = find_task(snapshot, id, clock)
        .ok_or_else(|| ToolError::internal("describing the task", "task missing after change"))?;
    Ok(Json(TaskResult { time_zone: clock.zone_name(), task: task_out(snapshot, &task, planned, clock) }))
}

fn quest_result(snapshot: &Snapshot, id: EntityId, clock: &Clock) -> Out<QuestResult> {
    let quest =
        snapshot.quest(id).ok_or_else(|| ToolError::internal("describing the quest", "quest missing after change"))?;
    Ok(Json(QuestResult { quest: quest_out(snapshot, quest, clock.now) }))
}

fn milestone_result(snapshot: &Snapshot, id: EntityId) -> Out<MilestoneResult> {
    let milestone = snapshot
        .milestones
        .iter()
        .find(|milestone| milestone.id == id)
        .ok_or_else(|| ToolError::internal("describing the milestone", "milestone missing after change"))?;
    Ok(Json(MilestoneResult { milestone: milestone_out(milestone) }))
}

fn entry_result(snapshot: &Snapshot, id: EntityId, clock: &Clock) -> Out<EntryResult> {
    let entry = snapshot
        .time_entries
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| ToolError::internal("describing the session", "entry missing after change"))?;
    Ok(Json(EntryResult { time_zone: clock.zone_name(), entry: entry_out(snapshot, entry, clock) }))
}

fn archive_result(snapshot: &Snapshot, kind: EntityKind, id: EntityId, clock: &Clock) -> Out<ArchiveResult> {
    let archived = archived_out(snapshot, kind, id, clock)
        .ok_or_else(|| ToolError::internal("describing the archived item", "item missing from the archive"))?;
    let hint = format!("Undo with restore (kind {}, id {id})", super::error::kind_name(kind).replace(' ', "_"));
    Ok(Json(ArchiveResult { archived, hint }))
}

#[tool_router(router = tool_router, vis = "pub(super)")]
impl QuestMcp {
    // Reading.

    #[tool(
        name = "list_tasks",
        description = "Tasks of one day, a range of days (up to 62) or the backlog (someday), in the apps' order \
            (open first). Without arguments: today. Includes repeats planned for those days (planned_repeat: \
            true); their ids work with every task tool.",
        annotations(title = "List tasks", read_only_hint = true, open_world_hint = false)
    )]
    async fn list_tasks(
        &self,
        Parameters(input): Parameters<ListTasksInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<TasksOut>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        read::list_tasks(&self.snapshot(&call).await?, &input, &call.clock).map(Json)
    }

    #[tool(
        name = "get_task",
        description = "One task with its quest, day, time block, repeat and tracked minutes.",
        annotations(title = "Get task", read_only_hint = true, open_world_hint = false)
    )]
    async fn get_task(
        &self,
        Parameters(input): Parameters<TaskIdInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<TaskResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        read::get_task(&self.snapshot(&call).await?, &input.task_id, &call.clock).map(Json)
    }

    #[tool(
        name = "list_quests",
        description = "Quests (long-term goals) with tracked minutes and open task count. Default: active ones.",
        annotations(title = "List quests", read_only_hint = true, open_world_hint = false)
    )]
    async fn list_quests(
        &self,
        Parameters(input): Parameters<ListQuestsInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<QuestsOut>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        Ok(Json(read::list_quests(&self.snapshot(&call).await?, &input, &call.clock)))
    }

    #[tool(
        name = "get_quest",
        description = "One quest with its milestones, open tasks (by day, backlog last) and the 20 most recently done.",
        annotations(title = "Get quest", read_only_hint = true, open_world_hint = false)
    )]
    async fn get_quest(
        &self,
        Parameters(input): Parameters<QuestIdInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<QuestDetailOut>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        read::get_quest(&self.snapshot(&call).await?, &input.quest_id, &call.clock).map(Json)
    }

    #[tool(
        name = "search",
        description = "Finds quests, tasks and milestones whose title or description contains all words of the \
            query (case-insensitive); with include_archived also archived items, which restore brings back.",
        annotations(title = "Search", read_only_hint = true, open_world_hint = false)
    )]
    async fn search(
        &self,
        Parameters(input): Parameters<SearchInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<SearchOut>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        read::search(&self.snapshot(&call).await?, &input, &call.clock).map(Json)
    }

    #[tool(
        name = "get_week_review",
        description = "The weekly review (Monday to Sunday): minutes of quest work per day and per quest, other \
            work, the weekly target and the quest share.",
        annotations(title = "Week review", read_only_hint = true, open_world_hint = false)
    )]
    async fn get_week_review(
        &self,
        Parameters(input): Parameters<WeekInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<WeekReviewOut>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        read::week_review(&self.snapshot(&call).await?, &input, &call.clock).map(Json)
    }

    #[tool(
        name = "get_running_timer",
        description = "The session the timer is tracking right now, or null. One timer per account: this \
            ignores `workspace` and names the workspace the session is in.",
        annotations(title = "Running timer", read_only_hint = true, open_world_hint = false)
    )]
    async fn get_running_timer(
        &self,
        Parameters(input): Parameters<ContextInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<TimerOut>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let snapshot = account::snapshot(&self.state, call.user, Scope::Timer).await?;
        Ok(Json(read::running_timer(&snapshot, &call.clock)))
    }

    #[tool(
        name = "list_time_entries",
        description = "Tracked sessions overlapping the given days (default today, up to 62 days), optionally of \
            one task or quest, with the total minutes within those days.",
        annotations(title = "List time entries", read_only_hint = true, open_world_hint = false)
    )]
    async fn list_time_entries(
        &self,
        Parameters(input): Parameters<ListEntriesInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<EntriesOut>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        read::list_entries(&self.snapshot(&call).await?, &input, &call.clock).map(Json)
    }

    #[tool(
        name = "list_workspaces",
        description = "The user's workspaces (separate sets of quests, tasks and time, e.g. work and private): id, \
            name, whether it is the default one, and what it holds. Every other tool works in the default \
            workspace unless given workspace (an id or exact name from here).",
        annotations(title = "List workspaces", read_only_hint = true, open_world_hint = false)
    )]
    async fn list_workspaces(
        &self,
        Parameters(_): Parameters<ListWorkspacesInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<WorkspacesOut>, ToolError> {
        let call = self.call(&parts, &CallContext::default())?;
        let snapshot = account::snapshot(&self.state, call.user, Scope::Account).await?;
        Ok(Json(WorkspacesOut { workspaces: snapshot.live_workspaces().map(workspace_out).collect() }))
    }

    // Tasks.

    #[tool(
        name = "create_task",
        description = "Creates a task at the end of a day's list (default today) or in the backlog (day \
            \"someday\"), optionally in a quest, with a time block and a repeat.",
        annotations(
            title = "Create task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_task(
        &self,
        Parameters(input): Parameters<CreateTaskInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<TaskResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let mut ids = EntityId::new;
        let (after, id) = self.change(&call, |s| write::create_task(s, &input, &call.clock, &mut ids)).await?;
        task_result(&after, id, &call.clock)
    }

    #[tool(
        name = "update_task",
        description = "Changes the given fields of a task: title, description, day, quest, time block, repeat. \
            Overwrites what was there. A repeat applies to the task's whole series.",
        annotations(
            title = "Update task",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_task(
        &self,
        Parameters(input): Parameters<UpdateTaskInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<TaskResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (after, id) = self.change(&call, |s| write::update_task(s, &input, &call.clock)).await?;
        task_result(&after, id, &call.clock)
    }

    #[tool(
        name = "complete_task",
        description = "Marks a task done (now). Already done: no change.",
        annotations(
            title = "Complete task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn complete_task(
        &self,
        Parameters(input): Parameters<TaskIdInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<TaskResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (after, id) = self.change(&call, |s| write::complete_task(s, &input.task_id, &call.clock)).await?;
        task_result(&after, id, &call.clock)
    }

    #[tool(
        name = "reopen_task",
        description = "Marks a done task open again. Already open: no change.",
        annotations(
            title = "Reopen task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn reopen_task(
        &self,
        Parameters(input): Parameters<TaskIdInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<TaskResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (after, id) = self.change(&call, |s| write::reopen_task(s, &input.task_id, &call.clock)).await?;
        task_result(&after, id, &call.clock)
    }

    #[tool(
        name = "move_task",
        description = "Moves a task to the top or bottom of a day's list, or to the backlog (day \"someday\").",
        annotations(
            title = "Move task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn move_task(
        &self,
        Parameters(input): Parameters<MoveTaskInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<TaskResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (after, id) = self.change(&call, |s| write::move_task(s, &input, &call.clock)).await?;
        task_result(&after, id, &call.clock)
    }

    #[tool(
        name = "archive_task",
        description = "Removes a task from lists into the archive (its tracked time stays). restore brings it back.",
        annotations(
            title = "Archive task",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn archive_task(
        &self,
        Parameters(input): Parameters<TaskIdInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<ArchiveResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (after, id) = self.change(&call, |s| write::archive_task(s, &input.task_id, &call.clock)).await?;
        archive_result(&after, EntityKind::Task, id, &call.clock)
    }

    #[tool(
        name = "restore",
        description = "Brings an archived task, quest (with its milestones), milestone or time entry back.",
        annotations(
            title = "Restore",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn restore(
        &self,
        Parameters(input): Parameters<RestoreInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<RestoreResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (_, (_, id)) = self.change(&call, |_| write::restore(&input)).await?;
        Ok(Json(RestoreResult { restored: input.kind, id: id.to_string() }))
    }

    // Quests and milestones.

    #[tool(
        name = "create_quest",
        description = "Creates a quest (a long-term goal), with the next unused color unless one is given.",
        annotations(
            title = "Create quest",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_quest(
        &self,
        Parameters(input): Parameters<CreateQuestInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<QuestResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let mut ids = EntityId::new;
        let (after, id) = self.change(&call, |s| write::create_quest(s, &input, &mut ids)).await?;
        quest_result(&after, id, &call.clock)
    }

    #[tool(
        name = "update_quest",
        description = "Changes the given fields of a quest: title, description, color, status (completed or \
            active). Overwrites what was there.",
        annotations(
            title = "Update quest",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_quest(
        &self,
        Parameters(input): Parameters<UpdateQuestInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<QuestResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (after, id) = self.change(&call, |s| write::update_quest(s, &input)).await?;
        quest_result(&after, id, &call.clock)
    }

    #[tool(
        name = "complete_quest",
        description = "Marks a quest completed; the apps list it under archived quests. update_quest with status \
            active reopens it. Its tasks and time stay.",
        annotations(
            title = "Complete quest",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn complete_quest(
        &self,
        Parameters(input): Parameters<QuestIdInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<QuestResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (after, id) = self.change(&call, |s| write::complete_quest(s, &input.quest_id)).await?;
        quest_result(&after, id, &call.clock)
    }

    #[tool(
        name = "add_milestone",
        description = "Records a milestone (a notable achievement) of a quest, on a day (default today).",
        annotations(
            title = "Add milestone",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_milestone(
        &self,
        Parameters(input): Parameters<AddMilestoneInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<MilestoneResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let mut ids = EntityId::new;
        let (after, id) = self.change(&call, |s| write::add_milestone(s, &input, &call.clock, &mut ids)).await?;
        milestone_result(&after, id)
    }

    #[tool(
        name = "update_milestone",
        description = "Changes a milestone's title or day.",
        annotations(
            title = "Update milestone",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_milestone(
        &self,
        Parameters(input): Parameters<UpdateMilestoneInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<MilestoneResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (after, id) = self.change(&call, |s| write::update_milestone(s, &input)).await?;
        milestone_result(&after, id)
    }

    #[tool(
        name = "archive_milestone",
        description = "Removes a milestone into the archive. restore brings it back.",
        annotations(
            title = "Archive milestone",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn archive_milestone(
        &self,
        Parameters(input): Parameters<MilestoneIdInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<ArchiveResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let (after, id) = self.change(&call, |s| write::delete_milestone(s, &input.milestone_id, &call.clock)).await?;
        archive_result(&after, EntityKind::Milestone, id, &call.clock)
    }

    // Time.

    #[tool(
        name = "start_timer",
        description = "Starts tracking time on a task now; a running timer is stopped first, in any workspace.",
        annotations(
            title = "Start timer",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn start_timer(
        &self,
        Parameters(input): Parameters<TaskIdInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<EntryResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let mut ids = EntityId::new;
        let (after, id) = self.change(&call, |s| write::start_timer(s, &input.task_id, &call.clock, &mut ids)).await?;
        entry_result(&after, id, &call.clock)
    }

    #[tool(
        name = "stop_timer",
        description = "Stops the running timer now, in whichever workspace it runs (this ignores `workspace`). \
            None running: no change (stopped is null).",
        annotations(
            title = "Stop timer",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn stop_timer(
        &self,
        Parameters(input): Parameters<ContextInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<StopOut>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let stop = |s: &Snapshot| Ok(write::stop_timer(s, &call.clock));
        let (after, stopped) = account::change(&self.state, call.user, Scope::Timer, call.clock.now, stop).await?;
        let stopped = stopped
            .and_then(|id| after.time_entries.iter().find(|entry| entry.id == id))
            .map(|entry| entry_out(&after, entry, &call.clock));
        let workspace = stopped.as_ref().map(|_| workspace_ref(after.active_workspace()));
        Ok(Json(StopOut { time_zone: call.clock.zone_name(), stopped, workspace }))
    }

    #[tool(
        name = "log_work",
        description = "Records finished work after the fact: a session from start to end on a day (default \
            today), for an existing task or a new one (created done). The session counts towards the task's quest.",
        annotations(
            title = "Log work",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn log_work(
        &self,
        Parameters(input): Parameters<LogWorkInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<EntryResult>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        let mut ids = EntityId::new;
        let (after, id) = self.change(&call, |s| write::log_work(s, &input, &call.clock, &mut ids)).await?;
        entry_result(&after, id, &call.clock)
    }

    // AI capture.

    #[tool(
        name = "capture_text",
        description = "Lets the server's AI turn free text into quests, tasks and logged work. By default only \
            returns the proposal; with apply true it creates everything in it, all or nothing. Prefer the other \
            tools when you can structure the text yourself.",
        annotations(
            title = "Capture text",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn capture_text(
        &self,
        Parameters(input): Parameters<CaptureInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<CaptureOut>, ToolError> {
        let call = self.call(&parts, &input.context)?;
        capture::capture_text(&self.state, call.user, &input, &call.clock).await.map(Json)
    }
}
