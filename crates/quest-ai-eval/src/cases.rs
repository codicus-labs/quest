//! The eval set: a fixed planner (quests and tasks around a fixed "now") and synthetic dictations
//! with the proposal each should produce. Loaded from `cases.json`.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc, Weekday};
use quest_domain::{EntityId, Frequency, MonthDay};
use quest_protocol::{CAPTURE_VERSION, CaptureContext, CaptureRequest, ContextQuest, ContextTask};
use serde::{Deserialize, Deserializer};
use uuid::Uuid;

/// The eval set shipped with the crate.
pub const CASES_JSON: &str = include_str!("../cases.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalSet {
    pub now: DateTime<Utc>,
    pub time_zone: String,
    pub language: String,
    /// By key, so cases can name them.
    pub quests: BTreeMap<String, FixtureQuest>,
    pub tasks: BTreeMap<String, FixtureTask>,
    pub cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureQuest {
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureTask {
    pub title: String,
    /// Key of a quest.
    pub quest: Option<String>,
    pub day: Option<NaiveDate>,
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub text: String,
    pub expect: Expect,
}

/// What the proposal must contain. A list that is left out is not checked; an empty list means
/// "nothing of this kind". Within an item, a field left out is not checked and `null` means "none".
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    pub quests: Option<Vec<ExpectQuest>>,
    pub tasks: Option<Vec<ExpectTask>>,
    pub work: Option<Vec<ExpectWork>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectQuest {
    /// The title contains one of these (case-insensitive).
    pub title: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectTask {
    pub title: Vec<String>,
    #[serde(default, deserialize_with = "present")]
    pub day: Option<Option<OneOf<NaiveDate>>>,
    /// A quest key, `"new"` for a quest the proposal creates, or `null`.
    #[serde(default, deserialize_with = "present")]
    pub quest: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    pub block: Option<Option<ExpectBlock>>,
    #[serde(default, deserialize_with = "present")]
    pub repeat: Option<Option<ExpectRepeat>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectBlock {
    /// HH:MM
    pub start: String,
    pub minutes: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectRepeat {
    pub every: u32,
    /// day, week, month or year.
    pub frequency: Frequency,
    /// mon … sun; left out: not checked.
    pub weekdays: Option<Vec<Weekday>>,
    /// `{ "day": 15 }` or `"last"`; left out: not checked.
    pub month_day: Option<MonthDay>,
    /// The end date; left out: not checked.
    pub until: Option<OneOf<NaiveDate>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectWork {
    pub task: ExpectWorkTask,
    pub day: NaiveDate,
    /// HH:MM
    pub start: String,
    /// HH:MM, `00:00` for midnight.
    pub end: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ExpectWorkTask {
    /// Key of a task in the fixture.
    Existing(String),
    /// A new task whose title contains one of these.
    New { new: Vec<String> },
}

/// One value, or a list of acceptable ones (for ambiguous phrases such as "next week").
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOf<T> {
    One(T),
    Any(Vec<T>),
}

impl<T: PartialEq> OneOf<T> {
    pub fn accepts(&self, value: &T) -> bool {
        match self {
            OneOf::One(expected) => expected == value,
            OneOf::Any(expected) => expected.contains(value),
        }
    }
}

/// Tells "left out" (outer `None`) from `null` (`Some(None)`).
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

impl EvalSet {
    pub fn load() -> Result<EvalSet, serde_json::Error> {
        serde_json::from_str(CASES_JSON)
    }

    /// Stable id of a fixture quest or task, so expectations can refer to it by key.
    pub fn id(key: &str) -> EntityId {
        EntityId(Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("quest-ai-eval/{key}").as_bytes()))
    }

    /// The context an app would send: every fixture quest and task.
    pub fn context(&self) -> CaptureContext {
        let quests = self
            .quests
            .iter()
            .map(|(key, quest)| ContextQuest {
                id: Self::id(key),
                title: quest.title.clone(),
                description: quest.description.clone(),
            })
            .collect();
        let tasks = self
            .tasks
            .iter()
            .map(|(key, task)| ContextTask {
                id: Self::id(key),
                title: task.title.clone(),
                quest_id: task.quest.as_deref().map(Self::id),
                day: task.day,
                done: task.done,
            })
            .collect();
        CaptureContext { quests, tasks }
    }

    pub fn request(&self, case: &Case) -> CaptureRequest {
        CaptureRequest {
            version: CAPTURE_VERSION,
            text: case.text.clone(),
            now: self.now,
            time_zone: self.time_zone.clone(),
            language: self.language.clone(),
            context: self.context(),
        }
    }
}

#[cfg(test)]
#[path = "cases_tests.rs"]
mod tests;
