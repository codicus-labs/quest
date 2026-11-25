use quest_domain::{Command, Placement, Position, TaskList};
use quest_view::plan;

use crate::commands;
use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::home::{Choice, home_task};
use crate::ids::{iso_day, parse_id, parse_iso_day};
use crate::plan::{DayWindow, PlanMemory};
use crate::time::Now;
use crate::types::{HomeTask, Moment};

#[derive(uniffi::Record)]
pub struct PlanSections {
    pub days: Vec<PlanDaySection>,
    pub later: Vec<PlanMonthSection>,
    pub done_today: Vec<HomeTask>,
    pub not_planned: Vec<HomeTask>,
    pub weeks: Vec<PlanWeek>,
}

#[derive(uniffi::Record)]
pub struct PlanDaySection {
    pub date: String,
    pub tasks: Vec<HomeTask>,
    pub due: Vec<PlanDueMarker>,
}

#[derive(uniffi::Record)]
pub struct PlanDueMarker {
    pub task_id: String,
    pub title: String,
}

#[derive(uniffi::Record)]
pub struct PlanMonthSection {
    pub start: String,
    pub partial: bool,
    pub days: Vec<PlanDaySection>,
}

#[derive(uniffi::Record)]
pub struct PlanWeek {
    pub days: Vec<PlanWeekDay>,
}

#[derive(uniffi::Record)]
pub struct PlanWeekDay {
    pub date: String,
    pub has_tasks: bool,
    pub jump_enabled: bool,
}

#[derive(uniffi::Enum)]
pub enum PlanTaskList {
    Today,
    Day { date: String },
    Inbox,
    Quest { quest_id: String },
}

#[uniffi::export]
impl QuestCore {
    /// Read the Plan list and register its recurrence previews for editing until `leave_plan`.
    pub fn plan_sections(&self, now: Moment) -> Result<PlanSections, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let today = now.today();
            let snapshot = self.store().snapshot();
            let plan = plan::build(&snapshot, today, now.at, &now.tz);
            self.remember_plan(PlanMemory {
                window: DayWindow { first: today, count: plan.days.len() as u32 },
                at: now.at,
                choice: Choice::default(),
            });
            let week_rows = plan::weeks(&plan, today);
            let task = |task: &quest_domain::Task| home_task(&snapshot, task, now.at, today);
            let day = |day: plan::Day<'_>| PlanDaySection {
                date: iso_day(day.date),
                tasks: day.tasks.iter().map(|row| task(row)).collect(),
                due: day
                    .due
                    .iter()
                    .map(|row| PlanDueMarker { task_id: row.id.to_string(), title: row.title.clone() })
                    .collect(),
            };
            Ok(PlanSections {
                days: plan.days.into_iter().map(day).collect(),
                later: plan
                    .later
                    .into_iter()
                    .map(|month| PlanMonthSection {
                        start: iso_day(month.start),
                        partial: month.partial,
                        days: month.days.into_iter().map(day).collect(),
                    })
                    .collect(),
                done_today: plan.done_today.into_iter().map(task).collect(),
                not_planned: plan.not_planned.into_iter().map(task).collect(),
                weeks: week_rows
                    .into_iter()
                    .map(|week| PlanWeek {
                        days: week
                            .into_iter()
                            .map(|day| PlanWeekDay {
                                date: iso_day(day.date),
                                has_tasks: day.has_tasks,
                                jump_enabled: day.jump_enabled,
                            })
                            .collect(),
                    })
                    .collect(),
            })
        })
    }

    /// Insert before `before_task_id`, or append if absent. Moving between days also changes the plan date.
    pub fn place_task(
        &self,
        task_id: String,
        list: PlanTaskList,
        before_task_id: Option<String>,
        now: Moment,
    ) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let task = self.task(&task_id)?;
            let file_today = matches!(&list, PlanTaskList::Day { date } if *date == iso_day(now.today()))
                && task.planned_on.is_none()
                && TaskList::Today.contains(&task, now.today());
            let list = match list {
                PlanTaskList::Today => TaskList::Today,
                PlanTaskList::Day { date } => {
                    let date = parse_iso_day(&date)?;
                    if date == now.today() { TaskList::Today } else { TaskList::Day(date) }
                }
                PlanTaskList::Inbox => TaskList::Inbox,
                PlanTaskList::Quest { quest_id } => TaskList::Quest(parse_id(&quest_id)?),
            };
            let position =
                before_task_id.as_deref().map(parse_id).transpose()?.map_or(Position::Bottom, Position::Before);
            let command = Command::PlaceTask {
                id: task.id,
                placement: Placement::InList { list, position },
                today: now.today(),
                at: now.at,
            };
            if file_today && let Some(plan) = commands::set_planned(&task, Some(now.today()), now) {
                let before = self.store().snapshot();
                self.edit(|store| {
                    store.dispatch_all(&[plan, command]).map_err(|e| MobileError::Rejected { reason: e.to_string() })
                })?;
                self.remember_task_date_edit(&task, &before, Some(list), now);
                return Ok(());
            }
            self.change_task_dates(&task, Some(command), now)
        })
    }
}
