//! The redesigned lists, the Quests tab, the Tracked tab and the mini player, mapped from the
//! snapshot with `quest-view`'s rules (`lists`, `row_meta`, `tracked`, `player`). Pure.

use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{Command, EntityId, Quest, Snapshot, Task};
use quest_view::player::Player;
use quest_view::quests::QuestsTab;
use quest_view::row_meta::{self, Tone};
use quest_view::tracked::{self, BlockKind, WeekBalance};
use quest_view::{format, lists};

use crate::error::MobileError;
use crate::home::{chip, color_hex};
use crate::ids::{iso_day, parse_id};
use crate::list_types::{
    ListKind, ListRow, ListSection, MarkTone, PlayerCard, QuestNav, QuestNavRow, RowMark, TaskList, TrackedBlock,
    TrackedData, WeekQuestLine, WeekSummary,
};
use crate::quick_add;
use crate::time::Now;
use crate::types::UndoToast;

pub(crate) fn task_list<'a>(
    snapshot: &'a Snapshot,
    kind: &ListKind,
    now: Now,
    toast: Option<UndoToast>,
    lingering: impl Fn(&Task) -> Option<&'a Task>,
) -> Result<TaskList, MobileError> {
    let today = now.today();
    let rows = RowContext { snapshot, today, at: now.at, running: running_task(snapshot) };
    let list = match kind {
        ListKind::Today => {
            let tracked = tracked::day_timeline(snapshot, today, &now.tz, now.at).tracked_ms;
            let subtitle = format!("{} · {} tracked", format::long_day(today), format::hours(tracked));
            let open = lists::today_with_linger(snapshot, today, &lingering);
            let done: Vec<_> = lists::done_today(snapshot, today, &now.tz)
                .into_iter()
                .filter(|task| lingering(task).is_none() || !open.iter().any(|row| row.id == task.id))
                .collect();
            open_and_done(
                kind,
                "Today".to_owned(),
                subtitle,
                rows.open(&open, false, &lingering),
                rows.all(&done, false),
            )
        }
        ListKind::Inbox => {
            let open = lists::inbox_with_linger(snapshot, &lingering);
            open_and_done(kind, "Inbox".to_owned(), String::new(), rows.open(&open, false, &lingering), Vec::new())
        }
        ListKind::Logbook => {
            let sections = lists::logbook_days(snapshot, &now.tz)
                .into_iter()
                .map(|(day, tasks)| ListSection { label: Some(format::long_day(day)), rows: rows.all(&tasks, false) })
                .collect();
            TaskList {
                kind: kind.clone(),
                title: "Logbook".to_owned(),
                subtitle: String::new(),
                quest: None,
                ring: 0.0,
                sections,
                empty: false,
                toast: None,
            }
        }
        ListKind::Quest { quest_id } => {
            let quest = known_quest(snapshot, quest_id)?;
            let week = tracked::week_balance(snapshot, today, &now.tz, now.at);
            let open = lists::quest_open_with_linger(snapshot, quest.id, &lingering);
            let done: Vec<_> = lists::quest_done(snapshot, quest.id)
                .into_iter()
                .filter(|task| lingering(task).is_none() || !open.iter().any(|row| row.id == task.id))
                .collect();
            let week_ms = week.quest_ms_of(quest.id);
            let subtitle = format!("{} this week · {} open", format::hours(week_ms), open.len());
            let mut list = open_and_done(
                kind,
                quest.title.clone(),
                subtitle,
                rows.open(&open, true, &lingering),
                rows.all(&done, true),
            );
            list.quest = Some(chip(quest));
            list.ring = tracked::fraction(week_ms, week.target_ms);
            list
        }
    };
    Ok(TaskList { toast, ..list })
}

/// Open rows without a label, then "n done" when something is.
fn open_and_done(kind: &ListKind, title: String, subtitle: String, open: Vec<ListRow>, done: Vec<ListRow>) -> TaskList {
    let empty = open.is_empty();
    let mut sections = vec![ListSection { label: None, rows: open }];
    if !done.is_empty() {
        sections.push(ListSection { label: Some(format!("{} done", done.len())), rows: done });
    }
    TaskList { kind: kind.clone(), title, subtitle, quest: None, ring: 0.0, sections, empty, toast: None }
}

/// What every row of one list needs to know.
struct RowContext<'a> {
    snapshot: &'a Snapshot,
    today: NaiveDate,
    at: DateTime<Utc>,
    /// The task whose timer runs here, and since when.
    running: Option<(EntityId, DateTime<Utc>)>,
}

impl RowContext<'_> {
    /// On a quest's own list the dot would repeat the header, and a later plan date is worth showing.
    fn all(&self, tasks: &[&Task], on_quest_list: bool) -> Vec<ListRow> {
        tasks.iter().map(|task| self.row(task, on_quest_list)).collect()
    }

    fn open<'a>(
        &self,
        tasks: &[&Task],
        on_quest_list: bool,
        lingering: impl Fn(&Task) -> Option<&'a Task>,
    ) -> Vec<ListRow> {
        tasks
            .iter()
            .map(|task| {
                let mut row = self.row(task, on_quest_list);
                if let Some(before) = lingering(task) {
                    row.marks = row_meta::date_marks(before, self.today, on_quest_list).into_iter().map(mark).collect();
                }
                row
            })
            .collect()
    }

    fn row(&self, task: &Task, on_quest_list: bool) -> ListRow {
        let running_since = self.running.filter(|(id, _)| *id == task.id).map(|(_, since)| since);
        let quest = if on_quest_list { None } else { task.quest_id.and_then(|q| self.snapshot.quest(q)).map(chip) };
        ListRow {
            id: task.id.to_string(),
            title: task.title.clone(),
            done: task.is_done(),
            quest,
            marks: row_meta::date_marks(task, self.today, on_quest_list).into_iter().map(mark).collect(),
            checklist_progress: crate::checklist::progress(self.snapshot, task.id),
            timer_running: running_since.is_some(),
            timer: running_since.map(|since| format::timer((self.at - since).num_milliseconds())),
            can_start_timer: !task.is_done(),
        }
    }
}

fn mark(mark: row_meta::DateMark) -> RowMark {
    let tone = match mark.tone {
        Tone::Overdue => MarkTone::Overdue,
        Tone::Accent => MarkTone::Accent,
        Tone::Muted => MarkTone::Muted,
    };
    RowMark { text: mark.text, tone }
}

fn running_task(snapshot: &Snapshot) -> Option<(EntityId, DateTime<Utc>)> {
    match quest_view::player::player(snapshot) {
        Player::Running { task: Some(task), started_at, .. } => Some((task, started_at)),
        _ => None,
    }
}

fn known_quest<'a>(snapshot: &'a Snapshot, quest_id: &str) -> Result<&'a Quest, MobileError> {
    let id = parse_id(quest_id)?;
    snapshot.quest(id).ok_or_else(|| MobileError::UnknownQuest { id: quest_id.to_owned() })
}

/// A new task's commands with the list's defaults: Today plans it today, a quest's list gives it
/// that quest, the Inbox and the Logbook nothing (an unplanned task without a quest is in the Inbox).
pub(crate) fn add(
    snapshot: &Snapshot,
    kind: &ListKind,
    id: EntityId,
    title: &str,
    now: Now,
) -> Result<Vec<Command>, MobileError> {
    let today = iso_day(now.today());
    let (quest, day) = match kind {
        ListKind::Today => (None, Some(today.as_str())),
        ListKind::Quest { quest_id } => (Some(known_quest(snapshot, quest_id)?.id.to_string()), None),
        ListKind::Inbox | ListKind::Logbook => (None, None),
    };
    quick_add::commands(snapshot, id, title, "", quest.as_deref(), day)
}

pub(crate) fn quest_nav(snapshot: &Snapshot, now: Now) -> QuestNav {
    let week = tracked::week_balance(snapshot, now.today(), &now.tz, now.at);
    let rows = |tab| quest_view::quests::listed(snapshot, tab).map(|q| nav_row(q, &week)).collect();
    QuestNav { active: rows(QuestsTab::Active), completed: rows(QuestsTab::Completed) }
}

fn nav_row(quest: &Quest, week: &WeekBalance) -> QuestNavRow {
    let ms = week.quest_ms_of(quest.id);
    let hours = format::hours(ms);
    QuestNavRow {
        id: quest.id.to_string(),
        title: quest.title.clone(),
        color_hex: color_hex(quest.color).to_owned(),
        a11y_label: format!("{}, {hours} this week", quest.title),
        week_hours: hours,
        ring: tracked::fraction(ms, week.target_ms),
    }
}

pub(crate) fn tracked(snapshot: &Snapshot, now: Now, focus_exit: &quest_view::focus_exit::FocusExit) -> TrackedData {
    tracked_with_total(snapshot, now, focus_exit, format::hours)
}

pub(crate) fn tracked_with_total(
    snapshot: &Snapshot,
    now: Now,
    focus_exit: &quest_view::focus_exit::FocusExit,
    total: fn(i64) -> String,
) -> TrackedData {
    let today = now.today();
    let day = tracked::day_timeline_with_focus(snapshot, today, &now.tz, now.at, focus_exit);
    let week = tracked::week_balance(snapshot, today, &now.tz, now.at);
    let clock = |minute: u32| format::hh_mm(minute);
    let blocks = day
        .blocks
        .into_iter()
        .map(|b| TrackedBlock {
            entry_id: b.entry.to_string(),
            range: format!(
                "{}–{}",
                clock(b.start_minute),
                if b.running { "now".to_owned() } else { clock(b.end_minute) }
            ),
            organizing: b.kind == BlockKind::Organizing,
            title: b.title,
            color_hex: b.color.map(|c| color_hex(c).to_owned()),
            running: b.running,
            start_minute: b.start_minute,
            end_minute: b.end_minute,
            outside_since_minute: b.outside_since_minute,
            rebooked: b.rebooked,
        })
        .collect();
    TrackedData {
        total: total(day.tracked_ms),
        first_hour: day.first_hour,
        last_hour: day.last_hour,
        blocks,
        now_minute: day.now_minute.unwrap_or(0),
        week: week_summary(&week),
    }
}

fn week_summary(week: &WeekBalance) -> WeekSummary {
    WeekSummary {
        total: format!("{} / {}", format::hours(week.quest_ms), format::hours(week.target_ms)),
        progress: tracked::fraction(week.quest_ms, week.target_ms),
        quests: week
            .quests
            .iter()
            .map(|q| WeekQuestLine {
                title: q.title.clone(),
                color_hex: color_hex(q.color).to_owned(),
                hours: format::hours(q.ms),
                fraction: tracked::fraction(q.ms, week.target_ms),
            })
            .collect(),
        organizing: format::hours(week.organizing_ms),
        no_quest: (week.no_quest_ms >= 60_000).then(|| format::hours(week.no_quest_ms)),
        postponed: u32::try_from(week.postponed).unwrap_or(u32::MAX),
    }
}

pub(crate) fn player(snapshot: &Snapshot, now: Now) -> PlayerCard {
    player_with_timer(snapshot, now, format::timer)
}

pub(crate) fn player_with_timer(snapshot: &Snapshot, now: Now, format: fn(i64) -> String) -> PlayerCard {
    let timer = |since: DateTime<Utc>| format((now.at - since).num_milliseconds());
    let quest_of = |id: Option<EntityId>| id.and_then(|q| snapshot.quest(q)).map(chip);
    match quest_view::player::player(snapshot) {
        Player::Running { task: Some(task), title, quest, started_at } => {
            PlayerCard::Running { task_id: task.to_string(), title, quest: quest_of(quest), timer: timer(started_at) }
        }
        // A session without a task shows as Organizing would: time and Stop, nothing to open.
        Player::Running { task: None, started_at, .. } => {
            PlayerCard::Organizing { timer: timer(started_at), back_to_task_id: None, back_to_title: None }
        }
        Player::Organizing { started_at, back_to } => PlayerCard::Organizing {
            timer: timer(started_at),
            back_to_task_id: back_to.as_ref().map(|t| t.task.to_string()),
            back_to_title: back_to.map(|t| t.title),
        },
        Player::Resume(task) => {
            let quest = snapshot.task(task.task).and_then(|t| t.quest_id);
            PlayerCard::Resume { task_id: task.task.to_string(), title: task.title, quest: quest_of(quest) }
        }
        Player::Empty => PlayerCard::Empty,
    }
}
