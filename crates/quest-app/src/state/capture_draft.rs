//! The proposal under review in the panel: `quest_capture::review::Draft` (selection, what applying
//! keeps, errors and labels) plus the title fields the user edits, written back as they change.

use std::collections::BTreeMap;

use quest_capture::review::Draft;
use quest_protocol::{CaptureProposal, QuestRef, TempKey, WorkTarget};
use rune::TextEdit;

pub struct Review {
    pub draft: Draft,
    /// The editable title of each new quest, task and new session task, by key.
    pub titles: BTreeMap<TempKey, TextEdit>,
    /// Why the last "Create" was refused.
    pub error: Option<String>,
}

impl Review {
    pub fn new(proposal: CaptureProposal) -> Review {
        let mut titles = BTreeMap::new();
        for quest in &proposal.quests {
            titles.insert(quest.key.clone(), TextEdit::new(&quest.title));
        }
        for task in &proposal.tasks {
            titles.insert(task.key.clone(), TextEdit::new(&task.title));
        }
        for work in &proposal.work {
            if let WorkTarget::New { title, .. } = &work.task {
                titles.insert(work.key.clone(), TextEdit::new(title));
            }
        }
        Review { draft: Draft::new(proposal), titles, error: None }
    }

    pub fn is_empty(&self) -> bool {
        let p = &self.draft.proposal;
        p.quests.is_empty() && p.tasks.is_empty() && p.work.is_empty()
    }

    pub fn toggle(&mut self, key: &TempKey) {
        let selected = self.draft.is_selected(key);
        self.draft.set_selected(key, !selected);
        self.error = None;
    }

    /// Writes the title fields into the proposal, so checks and applying see the edits.
    pub fn sync_titles(&mut self) {
        let title = |key: &TempKey| self.titles.get(key).map(|edit| edit.text.trim().to_owned());
        let p = &mut self.draft.proposal;
        for quest in &mut p.quests {
            if let Some(t) = title(&quest.key) {
                quest.title = t;
            }
        }
        for task in &mut p.tasks {
            if let Some(t) = title(&task.key) {
                task.title = t;
            }
        }
        for work in &mut p.work {
            if let (WorkTarget::New { title: target, .. }, Some(t)) = (&mut work.task, title(&work.key)) {
                *target = t;
            }
        }
        self.error = None;
    }

    /// Gives the task `key` an existing quest, a proposed one, or none; the choice is no longer a guess.
    pub fn set_task_quest(&mut self, key: &TempKey, quest: Option<QuestRef>) {
        if let Some(task) = self.draft.proposal.tasks.iter_mut().find(|t| &t.key == key) {
            task.quest = quest;
            task.quest_uncertain = false;
        }
        self.error = None;
    }

    /// "Created 1 quest and 2 tasks" for what `confirmed` holds.
    pub fn created_message(confirmed: &CaptureProposal) -> String {
        quest_capture::review::created_message(confirmed.quests.len(), confirmed.tasks.len(), confirmed.work.len())
    }
}
