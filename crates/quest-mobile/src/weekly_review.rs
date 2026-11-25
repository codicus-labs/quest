use quest_view::review::{Bucket, Review, Since, offset_week};

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::home::color_hex;
use crate::ids::iso_day;
use crate::time::Now;
use crate::types::Moment;

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct WeeklyReviewData {
    pub week_start: String,
    pub week_label: String,
    pub current: ReviewMetrics,
    pub previous: ReviewMetrics,
    pub goal_ms: i64,
    pub most_postponed: Vec<ReviewPostponed>,
    pub carried_over: Vec<ReviewSince>,
    pub overdue: Vec<ReviewSince>,
    pub tracked_ms_per_day: Vec<i64>,
    pub shares: Vec<ReviewShare>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ReviewMetrics {
    pub done: u64,
    pub added: u64,
    pub postponements: u64,
    pub quest_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ReviewPostponed {
    pub task_id: String,
    pub title: String,
    pub is_done: bool,
    pub count: u64,
    pub plan: Vec<Vec<String>>,
    pub deadline: Vec<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ReviewSince {
    pub task_id: String,
    pub title: String,
    pub since: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ReviewBucket {
    Quest { id: String, title: String, color_hex: String },
    Organizing,
    NoQuest,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ReviewShare {
    pub bucket: ReviewBucket,
    pub tracked_ms: i64,
    pub fraction: f64,
}

#[uniffi::export]
impl QuestCore {
    /// Offset from the local current week; -1 selects the previous week.
    pub fn weekly_review(&self, offset: i32, now: Moment) -> Result<WeeklyReviewData, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let week = offset_week(now.today(), offset).ok_or_else(|| MobileError::InvalidForm {
                reason: "Review week is out of range. Choose a week closer to today.".into(),
            })?;
            let snapshot = self.store().snapshot();
            let review = Review::build(&snapshot, week, &now.tz, now.at);
            let metrics = |tasks: &quest_domain::WeekTasks, quest_ms| ReviewMetrics {
                done: tasks.completed as u64,
                added: tasks.created as u64,
                postponements: tasks.postponements as u64,
                quest_ms,
            };
            let most_postponed = review
                .most_postponed
                .into_iter()
                .filter_map(|item| {
                    let task = snapshot.task(item.task_id)?;
                    let dates = |chains: Vec<Vec<_>>| {
                        chains.into_iter().map(|chain| chain.into_iter().map(iso_day).collect()).collect()
                    };
                    Some(ReviewPostponed {
                        task_id: task.id.to_string(),
                        title: task.title.clone(),
                        is_done: task.is_done(),
                        count: item.count as u64,
                        plan: dates(item.plan),
                        deadline: dates(item.deadline),
                    })
                })
                .collect();
            let since = |item: Since| {
                let task = snapshot.task(item.task_id)?;
                Some(ReviewSince { task_id: task.id.to_string(), title: task.title.clone(), since: iso_day(item.day) })
            };
            let shares = review
                .shares
                .into_iter()
                .filter_map(|share| {
                    let bucket = match share.bucket {
                        Bucket::Quest(id) => {
                            let quest = snapshot.quest(id)?;
                            ReviewBucket::Quest {
                                id: id.to_string(),
                                title: quest.title.clone(),
                                color_hex: color_hex(quest.color).into(),
                            }
                        }
                        Bucket::Organizing => ReviewBucket::Organizing,
                        Bucket::NoQuest => ReviewBucket::NoQuest,
                    };
                    Some(ReviewShare { bucket, tracked_ms: share.ms, fraction: share.fraction })
                })
                .collect();
            Ok(WeeklyReviewData {
                week_start: iso_day(week),
                week_label: quest_view::format::week_range(week),
                current: metrics(&review.tasks, review.time.quest_ms),
                previous: metrics(&review.previous_tasks, review.previous_quest_ms),
                goal_ms: review.time.target_ms,
                most_postponed,
                carried_over: review.carried_over.into_iter().filter_map(since).collect(),
                overdue: review.overdue.into_iter().filter_map(since).collect(),
                tracked_ms_per_day: review.time.total_ms_per_day.to_vec(),
                shares,
            })
        })
    }
}
