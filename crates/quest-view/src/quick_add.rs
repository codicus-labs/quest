use chrono::{DateTime, Datelike, Days, NaiveDate, Utc};
use quest_domain::{Command, DateField, EntityId, Position, Quest, Snapshot, TaskDates};

use crate::{calendar::set_date, nav::View, quest_picker::recent_quests};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fields {
    pub quest: Option<EntityId>,
    pub dates: TaskDates,
}

impl Fields {
    pub fn for_view(view: View, today: NaiveDate) -> Self {
        Self {
            quest: if let View::Quest(id) = view { Some(id) } else { None },
            dates: TaskDates { planned_on: (view == View::Today).then_some(today), due_on: None },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub title: String,
    pub fields: Fields,
}

impl Draft {
    pub fn command(self, id: EntityId, position: Position) -> Option<Command> {
        (!self.title.is_empty()).then_some(Command::CreateTask {
            id,
            title: self.title,
            quest_id: self.fields.quest,
            planned_on: self.fields.dates.planned_on,
            due_on: self.fields.dates.due_on,
            position,
        })
    }
}

pub fn resolve_day(word: &str, today: NaiveDate) -> Option<NaiveDate> {
    let offset = match word.to_ascii_lowercase().as_str() {
        "today" | "tod" => 0,
        "tom" | "tomorrow" => 1,
        word => {
            let weekday = match word {
                "mo" | "mon" | "monday" => 0,
                "tu" | "tue" | "tuesday" => 1,
                "we" | "wed" | "wednesday" => 2,
                "th" | "thu" | "thursday" => 3,
                "fr" | "fri" | "friday" => 4,
                "sa" | "sat" | "saturday" => 5,
                "su" | "sun" | "sunday" => 6,
                _ => return None,
            };
            (weekday + 7 - today.weekday().num_days_from_monday()) % 7
        }
    };
    today.checked_add_days(Days::new(u64::from(offset)))
}

pub fn quest_matches<'a>(quests: &[&'a Quest], query: &str) -> Vec<&'a Quest> {
    let query = query.to_lowercase();
    quests
        .iter()
        .copied()
        .filter(|q| q.title.to_lowercase().split_whitespace().any(|w| w.starts_with(&query)))
        .collect()
}

pub fn quest_token(input: &str, caret: usize) -> Option<(std::ops::Range<usize>, &str)> {
    let before = input.get(..caret)?;
    let after = input.get(caret..)?;
    let start = before
        .rfind(char::is_whitespace)
        .map_or(0, |offset| offset + before[offset..].chars().next().map_or(0, char::len_utf8));
    let end = caret + after.find(char::is_whitespace).unwrap_or(after.len());
    Some((start..end, before[start..].strip_prefix('#')?))
}

pub fn quest_suggestions<'a>(snapshot: &'a Snapshot, now: DateTime<Utc>, query: &str) -> Vec<&'a Quest> {
    quest_matches(&recent_quests(snapshot, now), query)
}

pub fn parse(input: &str, defaults: Fields, today: NaiveDate, quests: &[&Quest]) -> Draft {
    let mut fields = defaults;
    let mut title = Vec::new();
    for word in input.split_whitespace() {
        if let Some(query) = word.strip_prefix('#').filter(|q| !q.is_empty())
            && let Some(quest) = quest_matches(quests, query).first()
        {
            fields.quest = Some(quest.id);
            continue;
        }
        if let Some((field, day)) = [('@', DateField::Plan), ('!', DateField::Due)]
            .into_iter()
            .find_map(|(sigil, field)| Some((field, resolve_day(word.strip_prefix(sigil)?, today)?)))
        {
            fields.dates = set_date(fields.dates, field, Some(day));
            continue;
        }
        title.push(word);
    }
    Draft { title: title.join(" "), fields }
}

#[cfg(test)]
#[path = "quick_add_tests.rs"]
mod tests;
