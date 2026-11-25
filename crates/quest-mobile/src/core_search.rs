//! `QuestCore`'s weekly review and search. Memory-only; records and rules live in `review` and
//! `search`.

use quest_domain::EntityId;
use quest_view::undo::Deletable;

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::ids::parse_iso_day;
use crate::review_types::ReviewData;
use crate::search_types::{AppView, SearchMode, SearchOutcome, SearchResult, SearchToken};
use crate::time::Now;
use crate::types::Moment;
use crate::{commands, review, search};

#[uniffi::export]
impl QuestCore {
    /// The weekly review of the week containing `week` ("2026-11-24"); None: this week.
    pub fn review(&self, week: Option<String>, now: Moment) -> Result<ReviewData, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let day = week.as_deref().map(parse_iso_day).transpose()?.unwrap_or_else(|| now.today());
            Ok(review::review(&self.store().snapshot(), day, now))
        })
    }

    /// The entries for `query` in `mode`, in the desktop palette's order.
    pub fn search(&self, mode: SearchMode, query: String, now: Moment) -> Result<Vec<SearchResult>, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            search::search(&self.store().snapshot(), &mode, &query, now)
        })
    }

    /// What the field says in `mode`.
    pub fn search_placeholder(&self, mode: SearchMode) -> String {
        search::placeholder(&mode)
    }

    /// Carries out an entry's data part (create, move, delete, start the timer) and
    /// says what the UI shows next.
    pub fn run_search(&self, token: SearchToken, now: Moment) -> Result<SearchOutcome, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            Ok(match token {
                SearchToken::Go { view } => SearchOutcome::Show { view },
                SearchToken::ShowQuest { quest_id } => SearchOutcome::ShowQuest { quest_id },
                SearchToken::ToggleTheme => SearchOutcome::ToggleTheme,
                SearchToken::Account => SearchOutcome::Account,
                SearchToken::LogWork => SearchOutcome::LogWork,
                SearchToken::ManageWorkspaces => SearchOutcome::ManageWorkspaces,
                SearchToken::SwitchWorkspace { workspace_id } => {
                    self.switch_workspace(workspace_id)?;
                    SearchOutcome::Done
                }
                SearchToken::Choose { mode } => SearchOutcome::Choose { placeholder: search::placeholder(&mode), mode },
                SearchToken::CreateTask { title } => {
                    self.dispatch(search::create_task(EntityId::new(), &title, now.today())?)?;
                    SearchOutcome::Show { view: AppView::Home }
                }
                SearchToken::CreateQuest { title } => {
                    SearchOutcome::ShowQuest { quest_id: self.create_quest(title, None)? }
                }
                SearchToken::Focus { task_id } => {
                    let snapshot = self.store().snapshot();
                    let start = commands::start_timer(&snapshot, &self.task(&task_id)?, EntityId::new(), now)?;
                    self.dispatch_some(start)?;
                    SearchOutcome::Focus { task_id }
                }
                SearchToken::MoveTask { task_id, day } => {
                    let task = self.task(&task_id)?;
                    self.dispatch(search::move_task(task.id, day.as_deref(), now.at)?)?;
                    SearchOutcome::Done
                }
                SearchToken::DeleteTask { task_id } => {
                    let task = self.task(&task_id)?;
                    let item = Deletable::Task(task.id);
                    self.dispatch(item.delete(now.at))?;
                    self.undo_state().deleted(item, task.title, now.at);
                    SearchOutcome::Done
                }
            })
        })
    }
}

#[cfg(test)]
#[path = "core_search_tests.rs"]
mod tests;
