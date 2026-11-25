//! Scores a validated proposal against a case's expectation: one point per check (item found, each
//! expected field right, no extra items, clean titles). Pure.

use chrono::{NaiveTime, Weekday};
use quest_protocol::{CaptureProposal, ProposedRepeat, ProposedTask, ProposedWork, QuestRef, WorkTarget};

use crate::cases::{EvalSet, Expect, ExpectRepeat, ExpectTask, ExpectWork, ExpectWorkTask};

/// Dictation artefacts that must not end up in a title.
const FILLER_WORDS: [&str; 6] = ["ähm", "äh", "halt", "irgendwie", "also", "ne"];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Score {
    pub passed: u32,
    pub total: u32,
    /// What went wrong, for the report.
    pub failures: Vec<String>,
}

impl Score {
    pub fn is_hit(&self) -> bool {
        self.passed == self.total
    }

    fn check(&mut self, ok: bool, failure: impl FnOnce() -> String) {
        self.total += 1;
        if ok {
            self.passed += 1;
        } else {
            self.failures.push(failure());
        }
    }
}

pub fn score(expect: &Expect, proposal: &CaptureProposal) -> Score {
    let mut score = Score::default();
    if let Some(quests) = &expect.quests {
        let mut unmatched: Vec<&str> = proposal.quests.iter().map(|q| q.title.as_str()).collect();
        for quest in quests {
            let found = take_first(&mut unmatched, |title| matches_title(title, &quest.title));
            score.check(found.is_some(), || format!("missing quest {:?}", quest.title));
        }
        score.check(unmatched.is_empty(), || format!("extra quests {unmatched:?}"));
    }
    if let Some(tasks) = &expect.tasks {
        let mut unmatched: Vec<&ProposedTask> = proposal.tasks.iter().collect();
        for expected in tasks {
            match take_first(&mut unmatched, |task| matches_title(&task.title, &expected.title)) {
                Some(task) => {
                    score.check(true, String::new);
                    check_task(&mut score, expected, task, proposal);
                }
                None => fail_all(&mut score, task_checks(expected), format!("missing task {:?}", expected.title)),
            }
        }
        let extra: Vec<&str> = unmatched.iter().map(|task| task.title.as_str()).collect();
        score.check(extra.is_empty(), || format!("extra tasks {extra:?}"));
    }
    if let Some(work) = &expect.work {
        let mut unmatched: Vec<&ProposedWork> = proposal.work.iter().collect();
        for expected in work {
            match take_first(&mut unmatched, |item| matches_work_task(item, &expected.task)) {
                Some(item) => {
                    score.check(true, String::new);
                    check_work(&mut score, expected, item);
                }
                None => fail_all(&mut score, 3, format!("missing work on {:?}", expected.task)),
            }
        }
        score.check(unmatched.is_empty(), || format!("extra work logs: {}", unmatched.len()));
    }
    let titles = proposal.quests.iter().map(|q| &q.title).chain(proposal.tasks.iter().map(|t| &t.title));
    let dirty: Vec<&String> = titles.filter(|title| has_filler(title)).collect();
    score.check(dirty.is_empty(), || format!("filler words in {dirty:?}"));
    score
}

/// Counts `checks` failed checks plus the missing item itself.
fn fail_all(score: &mut Score, checks: u32, failure: String) {
    score.total += 1 + checks;
    score.failures.push(failure);
}

fn task_checks(expected: &ExpectTask) -> u32 {
    [expected.day.is_some(), expected.quest.is_some(), expected.block.is_some(), expected.repeat.is_some()]
        .into_iter()
        .map(u32::from)
        .sum()
}

fn check_task(score: &mut Score, expected: &ExpectTask, task: &ProposedTask, proposal: &CaptureProposal) {
    let title = &task.title;
    if let Some(day) = &expected.day {
        let ok = match (day, task.day) {
            (None, None) => true,
            (Some(day), Some(actual)) => day.accepts(&actual),
            _ => false,
        };
        score.check(ok, || format!("{title:?}: day {:?}, expected {day:?}", task.day));
    }
    if let Some(quest) = &expected.quest {
        let ok = match (quest.as_deref(), &task.quest) {
            (None, None) => true,
            (Some("new"), Some(QuestRef::New { key })) => proposal.quests.iter().any(|q| &q.key == key),
            (Some(key), Some(QuestRef::Existing { id })) => EvalSet::id(key) == *id,
            _ => false,
        };
        score.check(ok, || format!("{title:?}: quest {:?}, expected {quest:?}", task.quest));
    }
    if let Some(block) = &expected.block {
        let ok = match (block, task.block) {
            (None, None) => true,
            (Some(block), Some(actual)) => time(&block.start) == Some(actual.start) && block.minutes == actual.minutes,
            _ => false,
        };
        score.check(ok, || format!("{title:?}: block {:?}, expected {block:?}", task.block));
    }
    if let Some(repeat) = &expected.repeat {
        let ok = match (repeat, &task.repeat) {
            (None, None) => true,
            (Some(repeat), Some(actual)) => repeat_matches(repeat, actual),
            _ => false,
        };
        score.check(ok, || format!("{title:?}: repeat {:?}, expected {repeat:?}", task.repeat));
    }
}

fn repeat_matches(expected: &ExpectRepeat, actual: &ProposedRepeat) -> bool {
    let weekdays_ok = expected.weekdays.as_ref().is_none_or(|weekdays| {
        let sorted = |days: &[Weekday]| {
            let mut days = days.to_vec();
            days.sort_by_key(Weekday::num_days_from_monday);
            days
        };
        sorted(weekdays) == sorted(&actual.weekdays)
    });
    let month_day_ok = expected.month_day.is_none_or(|month_day| actual.month_day == Some(month_day));
    let until_ok = expected.until.as_ref().is_none_or(|until| actual.ends.is_some_and(|ends| until.accepts(&ends)));
    expected.every == actual.every && expected.frequency == actual.frequency && weekdays_ok && month_day_ok && until_ok
}

fn check_work(score: &mut Score, expected: &ExpectWork, work: &ProposedWork) {
    let label = format!("work {:?}", expected.task);
    score.check(work.day == expected.day, || format!("{label}: day {}, expected {}", work.day, expected.day));
    score.check(time(&expected.start) == Some(work.start), || {
        format!("{label}: start {}, expected {}", work.start.format("%H:%M"), expected.start)
    });
    score.check(time(&expected.end) == Some(work.end), || {
        format!("{label}: end {}, expected {}", work.end.format("%H:%M"), expected.end)
    });
}

fn matches_work_task(work: &ProposedWork, expected: &ExpectWorkTask) -> bool {
    match (expected, &work.task) {
        (ExpectWorkTask::Existing(key), WorkTarget::Existing { id }) => EvalSet::id(key) == *id,
        (ExpectWorkTask::New { new }, WorkTarget::New { title, .. }) => matches_title(title, new),
        _ => false,
    }
}

fn matches_title(title: &str, keywords: &[String]) -> bool {
    let title = title.to_lowercase();
    keywords.iter().any(|keyword| title.contains(&keyword.to_lowercase()))
}

fn has_filler(title: &str) -> bool {
    title.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|word| FILLER_WORDS.contains(&word))
}

fn time(text: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(text, "%H:%M").ok()
}

/// Removes and returns the first item that matches.
fn take_first<T>(items: &mut Vec<T>, matches: impl Fn(&T) -> bool) -> Option<T> {
    let index = items.iter().position(matches)?;
    Some(items.remove(index))
}

#[cfg(test)]
#[path = "score_tests.rs"]
mod tests;
