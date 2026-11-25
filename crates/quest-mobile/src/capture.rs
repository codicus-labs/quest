//! AI capture for mobile, pure: the proposal as an editable draft, how it reads, and what applying
//! it does, all through `quest_capture` like the desktop.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, NaiveTime, Utc, Weekday};
use quest_capture::review::{self, Draft};
use quest_capture::{CapturePlan, Created};
use quest_domain::{EntityId, MonthDay, QuestColor, Snapshot, Weekdays};
use quest_protocol::{
    CaptureProposal, ProposedQuest, ProposedRepeat, ProposedTask, ProposedWork, QuestRef, TempKey, WorkTarget,
};
use quest_store::CaptureError;
use quest_view::undo::TOAST_SECONDS;

use crate::capture_types::{
    CaptureApplied, CaptureDraft, CaptureView, DraftQuest, DraftQuestOption, DraftQuestRef, DraftQuestView, DraftTask,
    DraftTaskView, DraftWork, DraftWorkTask, DraftWorkView,
};
use crate::error::MobileError;
use crate::home::{chip, color_hex};
use crate::ids::{iso_day, parse_id, parse_iso_day};
use crate::repeat;
use crate::task_detail::day_value;
use crate::task_types::{DayValue, RepeatDraft, RepeatFrequency, RepeatPinChoice, RepeatWeekday};
use crate::time::Now;
use crate::types::QuestChip;

const DAY_MINUTES: u32 = 24 * 60;

/// The draft a fresh proposal opens with: everything selected.
pub(crate) fn draft(proposal: &CaptureProposal, snapshot: &Snapshot, now: Now) -> CaptureDraft {
    let quest = |quest: &Option<QuestRef>| quest.as_ref().map(quest_ref);
    CaptureDraft {
        quests: proposal
            .quests
            .iter()
            .map(|q| DraftQuest { key: q.key.0.clone(), selected: true, title: q.title.clone() })
            .collect(),
        tasks: proposal
            .tasks
            .iter()
            .map(|t| DraftTask {
                key: t.key.0.clone(),
                selected: true,
                title: t.title.clone(),
                description: t.description.clone(),
                quest: quest(&t.quest),
                quest_uncertain: t.quest_uncertain,
                planned_on: t.planned_on.map(day_value),
                due_on: t.due_on.map(day_value),
                repeat: t.repeat.as_ref().zip(t.planned_on).map(|(r, day)| repeat_draft(r, day, now)),
            })
            .collect(),
        work: proposal
            .work
            .iter()
            .map(|w| DraftWork {
                key: w.key.0.clone(),
                selected: true,
                task: match &w.task {
                    WorkTarget::Existing { id } => DraftWorkTask::Existing {
                        id: id.to_string(),
                        title: review::work_task_title(&w.task, snapshot).to_owned(),
                    },
                    WorkTarget::New { title, quest: q } => DraftWorkTask::New { title: title.clone(), quest: quest(q) },
                },
                uncertain: w.uncertain,
                day: day_value(w.day),
                start_minutes: review::minutes_of_day(w.start),
                end_minutes: review::minutes_of_day(w.end),
            })
            .collect(),
        dropped: proposal.dropped.iter().map(review::dropped_message).collect(),
    }
}

/// The draft as `quest_capture` reviews it, and per task the repeat's problem, if it has one.
struct Parsed {
    draft: Draft,
    repeat_errors: BTreeMap<String, String>,
}

fn parse(draft: &CaptureDraft, now: Now) -> Result<Parsed, MobileError> {
    let mut deselected = Vec::new();
    let mut keep = |key: &str, selected: bool| {
        if !selected {
            deselected.push(TempKey(key.to_owned()));
        }
        TempKey(key.to_owned())
    };
    let mut repeat_errors = BTreeMap::new();
    let mut quests = Vec::new();
    for q in &draft.quests {
        quests.push(ProposedQuest { key: keep(&q.key, q.selected), title: q.title.clone() });
    }
    let mut tasks = Vec::new();
    for t in &draft.tasks {
        let (planned_on, due_on) = (optional_day(&t.planned_on)?, optional_day(&t.due_on)?);
        let repeat = match (&t.repeat, planned_on) {
            (Some(draft), Some(day)) => {
                match repeat::rule(&RepeatDraft { starts: iso_day(day), ..draft.clone() }, now.today()) {
                    Ok(rule) => Some(ProposedRepeat {
                        every: rule.every,
                        frequency: rule.frequency,
                        weekdays: rule.weekdays.iter().collect(),
                        month_day: rule.month_day,
                        ends: rule.ends,
                    }),
                    Err(error) => {
                        repeat_errors.insert(t.key.clone(), error.to_string());
                        None
                    }
                }
            }
            _ => None,
        };
        tasks.push(ProposedTask {
            key: keep(&t.key, t.selected),
            title: t.title.clone(),
            description: t.description.clone(),
            quest: t.quest.as_ref().map(parse_quest_ref).transpose()?,
            quest_uncertain: t.quest_uncertain,
            planned_on,
            due_on,
            repeat,
        });
    }
    let mut work = Vec::new();
    for w in &draft.work {
        let task = match &w.task {
            DraftWorkTask::Existing { id, .. } => WorkTarget::Existing { id: parse_id(id)? },
            DraftWorkTask::New { title, quest } => {
                WorkTarget::New { title: title.clone(), quest: quest.as_ref().map(parse_quest_ref).transpose()? }
            }
        };
        work.push(ProposedWork {
            key: keep(&w.key, w.selected),
            task,
            uncertain: w.uncertain,
            day: parse_iso_day(&w.day.iso)?,
            start: clock(w.start_minutes)?,
            end: clock(w.end_minutes)?,
        });
    }
    let proposal = CaptureProposal { quests, tasks, work, dropped: Vec::new() };
    Ok(Parsed { draft: Draft { proposal, deselected: deselected.into_iter().collect() }, repeat_errors })
}

/// How the draft reads on screen.
pub(crate) fn view(draft: &CaptureDraft, snapshot: &Snapshot, now: Now) -> Result<CaptureView, MobileError> {
    let Parsed { draft: review, repeat_errors } = parse(draft, now)?;
    let today = now.today();
    let colors: BTreeMap<TempKey, QuestColor> = review.new_quest_colors(snapshot).into_iter().collect();
    let new_chip = |key: &TempKey| -> Option<QuestChip> {
        let quest = review.proposal.quests.iter().find(|q| q.key == *key)?;
        let color = colors.get(key)?;
        Some(QuestChip { id: key.0.clone(), name: quest.title.clone(), color_hex: color_hex(*color).to_owned() })
    };
    let quest_chip = |quest: &Option<QuestRef>| match review.effective_quest(quest)? {
        QuestRef::Existing { id } => snapshot.quest(*id).map(chip),
        QuestRef::New { key } => new_chip(key),
    };
    let quests = review
        .proposal
        .quests
        .iter()
        .map(|q| DraftQuestView {
            key: q.key.0.clone(),
            color_hex: colors.get(&q.key).map_or_else(|| color_hex(QuestColor::Blue), |c| color_hex(*c)).to_owned(),
            error: q.title.trim().is_empty().then(|| "Type a title.".to_owned()),
        })
        .collect();
    let tasks = review
        .proposal
        .tasks
        .iter()
        .map(|t| DraftTaskView {
            key: t.key.0.clone(),
            quest: quest_chip(&t.quest),
            when: review::task_when(t.planned_on, t.due_on, today),
            repeat_summary: draft
                .tasks
                .iter()
                .find(|d| d.key == t.key.0)
                .and_then(|d| d.repeat.as_ref())
                .filter(|_| t.planned_on.is_some())
                .map(|r| repeat::check(r, today).summary),
            error: review.task_error(t, snapshot).map(str::to_owned).or_else(|| repeat_errors.get(&t.key.0).cloned()),
        })
        .collect();
    let work = review
        .proposal
        .work
        .iter()
        .map(|w| {
            let quest = match &w.task {
                WorkTarget::Existing { .. } => {
                    review::work_task_quest(&w.task, snapshot).and_then(|id| snapshot.quest(id)).map(chip)
                }
                WorkTarget::New { quest, .. } => quest_chip(quest),
            };
            DraftWorkView {
                key: w.key.0.clone(),
                task_title: review::work_task_title(&w.task, snapshot).to_owned(),
                quest,
                when: review::work_when(w, today),
                start_label: quest_view::format::hh_mm(review::minutes_of_day(w.start)),
                end_label: quest_view::format::hh_mm(review::end_minutes(w.end)),
                duration: review::work_duration(w),
                error: review.work_error(w, snapshot).map(str::to_owned),
            }
        })
        .collect();
    let existing = quest_view::quest_picker::recent_quests(snapshot, now.at)
        .into_iter()
        .map(|q| DraftQuestOption { quest: DraftQuestRef::Existing { id: q.id.to_string() }, chip: chip(q) });
    let new = colors.keys().filter_map(|key| {
        Some(DraftQuestOption { quest: DraftQuestRef::New { key: key.0.clone() }, chip: new_chip(key)? })
    });
    let selected_count = review.selected_count();
    Ok(CaptureView {
        quests,
        tasks,
        work,
        quest_options: existing.chain(new).collect(),
        selected_count: selected_count as u32,
        can_apply: review.can_apply(snapshot)
            && repeat_errors.keys().all(|key| !review.is_selected(&TempKey(key.clone()))),
        apply_label: review::apply_label(selected_count),
    })
}

/// The commands that create what is selected, and the message to show afterwards.
pub(crate) fn apply(
    draft: &CaptureDraft,
    snapshot: &Snapshot,
    now: Now,
    new_id: impl FnMut() -> EntityId,
) -> Result<(CapturePlan, String), MobileError> {
    let Parsed { draft: review, repeat_errors } = parse(draft, now)?;
    if let Some(reason) = repeat_errors.iter().find(|(key, _)| review.is_selected(&TempKey((*key).clone()))) {
        return Err(MobileError::InvalidRepeat { reason: reason.1.clone() });
    }
    if review.selected_count() == 0 {
        return Err(MobileError::InvalidForm { reason: "Select something to create.".to_owned() });
    }
    let confirmed = review.confirmed();
    if let Some(error) = confirmed.tasks.iter().find_map(|t| review.task_error(t, snapshot)) {
        return Err(MobileError::InvalidForm { reason: error.to_owned() });
    }
    if let Some(error) = confirmed.work.iter().find_map(|w| review.work_error(w, snapshot)) {
        return Err(MobileError::InvalidForm { reason: error.to_owned() });
    }
    let plan = quest_capture::plan(&confirmed, snapshot, now.at, now.tz, new_id)
        .map_err(|e| MobileError::Rejected { reason: e.to_string() })?;
    let count = |kind: fn(&Created) -> bool| plan.created.iter().filter(|c| kind(c)).count();
    let sessions = count(|c| matches!(c, Created::Entry(_)));
    let quests = count(|c| matches!(c, Created::Quest(_)));
    // Tasks created for a session are part of it, not counted twice.
    let tasks = confirmed.tasks.len();
    Ok((plan, review::created_message(quests, tasks, sessions)))
}

/// The snackbar after applying: the message with Undo, shown as long as the desktop's toast.
pub(crate) fn applied(message: String) -> CaptureApplied {
    CaptureApplied { message, undo_label: "Undo".to_owned(), show_for_ms: (TOAST_SECONDS * 1000) as u64 }
}

/// How long after a capture its Undo still works: well past the snackbar, which accessibility
/// settings may keep up longer, but a forgotten capture is not undone much later.
pub(crate) const UNDO_LIMIT: chrono::Duration = chrono::Duration::minutes(2);

/// What the last applied capture created, for its Undo until `until`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CaptureUndo {
    created: Vec<Created>,
    until: DateTime<Utc>,
}

impl CaptureUndo {
    pub fn new(created: Vec<Created>, now: Now) -> CaptureUndo {
        CaptureUndo { created, until: now.at + UNDO_LIMIT }
    }

    /// The created items, unless the Undo came too late.
    pub fn created_if_open(self, now: Now) -> Option<Vec<Created>> {
        (now.at < self.until).then_some(self.created)
    }
}

pub(crate) fn capture_error(error: CaptureError) -> MobileError {
    match error {
        CaptureError::SignedOut => MobileError::AiSignedOut,
        CaptureError::SessionExpired => MobileError::SessionExpired,
        CaptureError::Disabled => MobileError::AiDisabled,
        CaptureError::NotConfigured => MobileError::AiNotConfigured,
        CaptureError::Unreachable { message } => MobileError::ServerUnreachable { detail: message },
        CaptureError::ProviderUnavailable => MobileError::AiUnavailable,
        CaptureError::InvalidResponse => MobileError::AiInvalidResponse,
        CaptureError::RateLimited => MobileError::AiRateLimited,
        CaptureError::TextTooLong => {
            MobileError::CaptureTextTooLong { max_chars: quest_protocol::MAX_CAPTURE_TEXT_CHARS as u32 }
        }
        CaptureError::Server { message } => MobileError::Rejected { reason: message },
    }
}

fn quest_ref(quest: &QuestRef) -> DraftQuestRef {
    match quest {
        QuestRef::Existing { id } => DraftQuestRef::Existing { id: id.to_string() },
        QuestRef::New { key } => DraftQuestRef::New { key: key.0.clone() },
    }
}

fn parse_quest_ref(quest: &DraftQuestRef) -> Result<QuestRef, MobileError> {
    Ok(match quest {
        DraftQuestRef::Existing { id } => QuestRef::Existing { id: parse_id(id)? },
        DraftQuestRef::New { key } => QuestRef::New { key: TempKey(key.clone()) },
    })
}

fn optional_day(day: &Option<DayValue>) -> Result<Option<NaiveDate>, MobileError> {
    day.as_ref().map(|d| parse_iso_day(&d.iso)).transpose()
}

fn clock(minutes: u32) -> Result<NaiveTime, MobileError> {
    NaiveTime::from_hms_opt(minutes / 60, minutes % 60, 0)
        .filter(|_| minutes < DAY_MINUTES)
        .ok_or_else(|| MobileError::InvalidForm { reason: "Pick a time between 00:00 and 23:59.".to_owned() })
}

/// The repeat dialog's fields for a proposed rule starting on `day`.
fn repeat_draft(repeat: &ProposedRepeat, day: chrono::NaiveDate, now: Now) -> RepeatDraft {
    RepeatDraft {
        every: repeat.every.to_string(),
        frequency: match repeat.frequency {
            quest_domain::Frequency::Day => RepeatFrequency::Day,
            quest_domain::Frequency::Week => RepeatFrequency::Week,
            quest_domain::Frequency::Month => RepeatFrequency::Month,
            quest_domain::Frequency::Year => RepeatFrequency::Year,
        },
        pin: RepeatPinChoice::None,
        weekdays: repeat
            .weekdays
            .iter()
            .fold(Weekdays::default(), |days, d| days.toggled(*d))
            .iter()
            .map(weekday)
            .collect(),
        last_day: repeat.month_day == Some(MonthDay::Last),
        month_day: match repeat.month_day {
            Some(MonthDay::Day(day)) => day.to_string(),
            _ => String::new(),
        },
        starts: iso_day(day),
        ends_on_date: repeat.ends.is_some(),
        ends: repeat.ends.map(iso_day).unwrap_or_default(),
        time_zone: now.tz.to_string(),
    }
}

fn weekday(day: Weekday) -> RepeatWeekday {
    match day {
        Weekday::Mon => RepeatWeekday::Mon,
        Weekday::Tue => RepeatWeekday::Tue,
        Weekday::Wed => RepeatWeekday::Wed,
        Weekday::Thu => RepeatWeekday::Thu,
        Weekday::Fri => RepeatWeekday::Fri,
        Weekday::Sat => RepeatWeekday::Sat,
        Weekday::Sun => RepeatWeekday::Sun,
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
