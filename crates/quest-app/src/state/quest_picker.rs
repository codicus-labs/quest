//! The quest picker: what it chooses a quest for, its search and the highlighted row. One is open
//! at a time. Its quests come from `quest_view::quest_picker`: most recently used first, and the
//! search keeps those whose title contains it.

use quest_domain::{Command, EntityId, QuestColor};
use quest_protocol::{QuestRef, TempKey};
use quest_view::quest_picker;
use rune::TextEdit;

use crate::state::{CaptureStep, State};

/// What the picker chooses a quest for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickFor {
    TaskModal,
    /// Log work's new task.
    LogWork,
    /// A task in the AI capture's proposal.
    CaptureTask(TempKey),
    /// The quick-add field's new task (Today).
    QuickAdd,
    /// Focus's task.
    Focus(EntityId),
}

impl PickFor {
    /// The key of the chip that opens the picker; focus goes back to it.
    pub fn trigger_key(&self) -> String {
        match self {
            PickFor::TaskModal => "task-modal-quest".into(),
            PickFor::LogWork => "log-work-quest".into(),
            PickFor::CaptureTask(key) => format!("capture-item-{}-quest", key.0),
            PickFor::QuickAdd => "quick-add-quest".into(),
            PickFor::Focus(_) => "focus-quest".into(),
        }
    }
}

/// A row's choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    NoQuest,
    Quest(EntityId),
    /// A new quest of the capture's proposal.
    New(TempKey),
}

impl Pick {
    /// The row's key, by what it chooses.
    pub fn key(&self) -> String {
        match self {
            Pick::NoQuest => "quest-picker-none".into(),
            Pick::Quest(id) => format!("quest-picker-{id}"),
            Pick::New(key) => format!("quest-picker-new-{}", key.0),
        }
    }
}

pub struct PickRow {
    pub pick: Pick,
    /// What the search matches: the quest's title, or "No quest".
    pub title: String,
    /// What the row shows: the title, "New: …" for a proposed quest.
    pub label: String,
    /// `None` for "No quest".
    pub color: Option<QuestColor>,
}

pub struct QuestPicker {
    pub target: PickFor,
    pub search: TextEdit,
    /// The row the arrow keys moved to; none is the first row.
    pub highlight: Option<Pick>,
}

impl State {
    /// Opens the picker for `target` on its current choice, or closes it if it is open there.
    pub fn toggle_quest_picker(&mut self, target: PickFor) {
        if self.quest_picker.as_ref().is_some_and(|picker| picker.target == target) {
            self.quest_picker = None;
            return;
        }
        self.task_calendar = None;
        self.quick.close_picker();
        let highlight = self.current_pick(&target).filter(|pick| *pick != Pick::NoQuest);
        let mut search = TextEdit::new("");
        // Asks Rune to focus the field once it shows.
        search.focused = true;
        self.quest_picker = Some(QuestPicker { target, search, highlight });
    }

    /// The search field's text; a scratch one once the picker closed (a blurred field still writes).
    pub fn quest_picker_search(&mut self) -> &mut TextEdit {
        match &mut self.quest_picker {
            Some(picker) => &mut picker.search,
            None => &mut self.closed_quest_search,
        }
    }

    /// The rows the search keeps, in order.
    pub fn quest_picker_rows(&self) -> Vec<PickRow> {
        let Some(picker) = &self.quest_picker else { return Vec::new() };
        let query = picker.search.text.as_str();
        self.pick_rows(&picker.target).into_iter().filter(|row| quest_picker::matches(&row.title, query)).collect()
    }

    /// The search changed: the first row the search keeps is highlighted.
    pub fn quest_search_changed(&mut self) {
        if let Some(picker) = &mut self.quest_picker {
            picker.highlight = None;
        }
    }

    /// Carries out `pick` for the open picker and closes it.
    pub fn choose_quest(&mut self, pick: Pick) {
        let Some(picker) = self.quest_picker.take() else { return };
        let quest = match &pick {
            Pick::Quest(id) => Some(*id),
            Pick::NoQuest | Pick::New(_) => None,
        };
        match picker.target {
            PickFor::TaskModal => self.assign_task_quest(quest),
            PickFor::LogWork => self.log_work.quest = quest,
            PickFor::CaptureTask(key) => {
                let quest = match pick {
                    Pick::NoQuest => None,
                    Pick::Quest(id) => Some(QuestRef::Existing { id }),
                    Pick::New(key) => Some(QuestRef::New { key }),
                };
                if let Some(review) = self.capture.review_mut() {
                    review.set_task_quest(&key, quest);
                }
            }
            PickFor::QuickAdd => crate::ui::quick_add::choose_quest(self, pick),
            PickFor::Focus(task) => {
                if self.plan_task(task).is_some_and(|t| t.quest_id != quest) {
                    self.dispatch(Command::AssignTask { id: task, quest_id: quest });
                }
            }
        }
    }

    /// What `target` has chosen now.
    pub fn current_pick(&self, target: &PickFor) -> Option<Pick> {
        let quest = |id: Option<EntityId>| Some(id.map_or(Pick::NoQuest, Pick::Quest));
        match target {
            PickFor::TaskModal => {
                let task = self.task_modal.as_ref().and_then(|form| self.plan_task(form.task))?;
                quest(task.quest_id)
            }
            PickFor::LogWork => quest(self.log_work.quest),
            PickFor::CaptureTask(key) => {
                let CaptureStep::Review(review) = &self.capture.step else { return None };
                let task = review.draft.proposal.tasks.iter().find(|t| &t.key == key)?;
                Some(match review.draft.effective_quest(&task.quest) {
                    None => Pick::NoQuest,
                    Some(QuestRef::Existing { id }) => Pick::Quest(*id),
                    Some(QuestRef::New { key }) => Pick::New(key.clone()),
                })
            }
            PickFor::QuickAdd => Some(crate::ui::quick_add::picked(self)),
            PickFor::Focus(task) => quest(self.plan_task(*task)?.quest_id),
        }
    }

    /// Every row for `target`: the active quests, most recently used first; then a capture's
    /// proposed quests; then "No quest" where a choice may have none.
    fn pick_rows(&self, target: &PickFor) -> Vec<PickRow> {
        let mut rows: Vec<_> = existing_rows(&self.snap, self.now).collect();
        if let (PickFor::CaptureTask(_), CaptureStep::Review(review)) = (target, &self.capture.step) {
            let colors = review.draft.new_quest_colors(&self.snap);
            for quest in review.draft.proposal.quests.iter().filter(|q| review.draft.is_selected(&q.key)) {
                let title = review.titles.get(&quest.key).map_or_else(String::new, |t| t.text.clone());
                rows.push(PickRow {
                    pick: Pick::New(quest.key.clone()),
                    label: format!("New: {title}"),
                    title,
                    color: colors.iter().find(|(key, _)| key == &quest.key).map(|(_, color)| *color),
                });
            }
        }
        rows.push(no_quest_row());
        rows
    }
}

pub fn existing_rows(
    snapshot: &quest_domain::Snapshot,
    now: chrono::DateTime<chrono::Utc>,
) -> impl Iterator<Item = PickRow> {
    quest_picker::recent_quests(snapshot, now).into_iter().map(|q| PickRow {
        pick: Pick::Quest(q.id),
        title: q.title.clone(),
        label: q.title.clone(),
        color: Some(q.color),
    })
}

pub fn no_quest_row() -> PickRow {
    PickRow { pick: Pick::NoQuest, title: "No quest".into(), label: "No quest".into(), color: None }
}
