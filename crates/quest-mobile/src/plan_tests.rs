use quest_domain::{Command, Position, Snapshot};

use super::*;
use crate::test_support::{at, berlin, day, id, quest, task};

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn now() -> Now {
    berlin("2026-11-25T10:00:00+01:00")
}

fn today() -> NaiveDate {
    day("2026-11-25")
}

fn win(first: &str, count: u32) -> DayWindow {
    DayWindow { first: day(first), count }
}

fn memory() -> PlanMemory {
    PlanMemory { window: win("2026-11-23", 14), at: now().at, choice: Choice::default() }
}

#[test]
fn a_fresh_window_covers_whole_weeks_with_two_weeks_either_side() {
    let window = window(today(), 5, None, today());
    assert_eq!(window, win("2026-11-09", 35), "Monday two weeks before to the Monday after 25 Nov + 5 + 14 days");
    assert!(window.first <= today() - Days::new(14));
    assert!(window.end() >= today() + Days::new(5 + 14));
}

#[test]
fn a_built_window_holds_until_fewer_than_three_days_remain_beyond_an_edge() {
    let built = window(today(), 5, None, today());
    assert_eq!(window(day("2026-11-12"), 5, Some(built), today()), built, "3 days before still built");
    assert_eq!(window(day("2026-12-06"), 5, Some(built), today()), built, "3 days after still built");
    let earlier = window(day("2026-11-11"), 5, Some(built), today());
    assert_eq!(earlier, win("2026-10-26", 35), "moves a whole week at a time");
    let later = window(day("2026-12-07"), 5, Some(built), today());
    assert_eq!(later.first, day("2026-11-23"));
}

#[test]
fn the_window_stays_within_three_years_of_today() {
    let earliest = today() - Days::new(REACH_DAYS);
    let end = today() + Days::new(REACH_DAYS + 1);
    let far_back = window(day("2000-01-01"), 5, None, today());
    assert_eq!(far_back.first, earliest);
    let far_ahead = window(day("2040-01-01"), 5, None, today());
    assert_eq!(far_ahead.end(), end);
    assert!(far_ahead.first < end && far_ahead.count > 0);
    assert_eq!(window(today(), 0, None, today()), window(today(), 1, None, today()), "at least one day shows");
}

#[test]
fn a_day_lists_quest_work_then_other_with_open_cards_first() {
    let mut snapshot = Snapshot::default();
    snapshot.quests.push(quest(1, "Work"));
    let mut done = task(10, "Done first", Some(1), Some("2026-11-26"));
    done.completed_at = Some(at("2026-11-25T09:00:00+01:00"));
    snapshot.tasks.extend([
        done,
        task(12, "Second", Some(1), Some("2026-11-26")),
        task(11, "First", Some(1), Some("2026-11-26")),
        task(13, "Errand", None, Some("2026-11-26")),
        task(14, "Gone quest", Some(9), Some("2026-11-26")),
        task(15, "Other day", None, Some("2026-11-27")),
    ]);
    let data = plan(&snapshot, win("2026-11-24", 3), now(), Choice::default(), None);
    assert_eq!(data.window, PlanWindow { first_day: "2026-11-24".into(), count: 3 });
    assert_eq!(data.today_iso, "2026-11-25");
    let thursday = &data.days[2];
    let titles = |cards: &[crate::types::HomeTask]| cards.iter().map(|c| c.title.clone()).collect::<Vec<_>>();
    assert_eq!(titles(&thursday.quest_work), ["First", "Second", "Done first"]);
    assert_eq!(titles(&thursday.other), ["Errand", "Gone quest"], "a deleted quest's task counts as other");
    assert!(data.days[0].quest_work.is_empty() && data.days[0].other.is_empty());
}

#[test]
fn days_carry_the_desktop_heads() {
    let data = plan(&Snapshot::default(), win("2026-11-24", 3), now(), Choice::default(), None);
    let [tuesday, wednesday, thursday] = &data.days[..] else { panic!("three days") };
    assert_eq!(
        (tuesday.day_iso.as_str(), tuesday.weekday.as_str(), tuesday.day_number.as_str()),
        ("2026-11-24", "Tue", "24")
    );
    assert_eq!(tuesday.label, "Tuesday, November 24");
    assert_eq!(tuesday.month_label, "November 2026");
    assert!(tuesday.is_past && !tuesday.is_today);
    assert!(wednesday.is_today && !wednesday.is_past);
    assert!(!thursday.is_today && !thursday.is_past);
}

#[test]
fn the_filter_narrows_quest_work_only() {
    let mut snapshot = Snapshot::default();
    snapshot.quests.extend([quest(1, "Work"), quest(2, "Home")]);
    snapshot.tasks.extend([
        task(10, "Work task", Some(1), Some("2026-11-25")),
        task(11, "Home task", Some(2), Some("2026-11-25")),
        task(12, "Errand", None, Some("2026-11-25")),
    ]);
    let choice = Choice { filter: Some(id(2)), pick: None };
    let data = plan(&snapshot, win("2026-11-25", 1), now(), choice, None);
    assert_eq!(data.quest_filter.map(|c| c.name), Some("Home".into()));
    assert_eq!(data.quests.len(), 2);
    let today = &data.days[0];
    assert_eq!(today.quest_work.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Home task"]);
    assert_eq!(today.other.len(), 1);
    let gone = plan(&snapshot, win("2026-11-25", 1), now(), Choice { filter: Some(id(9)), pick: None }, None);
    assert_eq!((gone.quest_filter, gone.days[0].quest_work.len()), (None, 2), "a vanished quest filters nothing");
}

#[test]
fn a_drop_moves_before_the_card_or_to_the_end() {
    let card = task(10, "Card", None, Some("2026-11-25"));
    let to = Some(day("2026-11-27"));
    let open = task(11, "Open", None, Some("2026-11-27"));
    let mut done = task(12, "Done", None, Some("2026-11-27"));
    done.completed_at = Some(at("2026-11-25T09:00:00+01:00"));
    let at = now().at;
    let moved = |position| Some(Command::MoveTask { id: id(10), planned_on: to, position, at });
    assert_eq!(drop_move(&card, to, Some(&open), at), moved(Position::Before(id(11))));
    assert_eq!(drop_move(&card, to, None, at), moved(Position::Bottom));
    assert_eq!(drop_move(&card, to, Some(&card), at), None, "dropped back where it was");
    assert_eq!(drop_move(&card, to, Some(&done), at), moved(Position::Bottom), "after the open cards");
    let mut finished = card.clone();
    finished.completed_at = done.completed_at;
    let finished_moved = Some(Command::MoveTask { id: id(10), planned_on: to, position: Position::Before(id(12)), at });
    assert_eq!(drop_move(&finished, to, Some(&done), at), finished_moved, "done among done");
}

#[test]
fn nudges_step_within_the_section_as_shown() {
    let mut snapshot = Snapshot::default();
    snapshot.quests.extend([quest(1, "Work"), quest(2, "Home")]);
    snapshot.tasks.extend([
        task(10, "A", Some(1), Some("2026-11-25")),
        task(11, "Errand", None, Some("2026-11-25")),
        task(12, "B", Some(2), Some("2026-11-25")),
        task(13, "C", Some(1), Some("2026-11-25")),
    ]);
    let c = snapshot.task(id(13)).unwrap().clone();
    let at = now().at;
    let up = |choice| nudge_move(&snapshot, at, choice, &c, true, at);
    let on_today = |position| Some(Command::MoveTask { id: id(13), planned_on: Some(today()), position, at });
    assert_eq!(up(Choice::default()), on_today(Position::Before(id(12))), "skips the other section");
    assert_eq!(up(Choice { filter: Some(id(1)), pick: None }), on_today(Position::Before(id(10))), "as filtered");
    let a = snapshot.task(id(10)).unwrap().clone();
    assert_eq!(nudge_move(&snapshot, at, Choice::default(), &a, true, at), None, "first stays");
    let unplanned = task(20, "Someday", None, None);
    assert_eq!(nudge_move(&snapshot, at, Choice::default(), &unplanned, false, at), None);
}

#[test]
fn shifts_go_to_the_bottom_of_the_next_or_previous_day() {
    let card = task(10, "Card", None, Some("2026-11-30"));
    let at = now().at;
    let bottom = |on: &str| Command::MoveTask { id: id(10), planned_on: Some(day(on)), position: Position::Bottom, at };
    assert_eq!(shift_move(&card, true, now()), bottom("2026-12-01"));
    assert_eq!(shift_move(&card, false, now()), bottom("2026-11-29"));
    assert_eq!(shift_move(&task(10, "Card", None, None), true, now()), bottom("2026-11-26"), "from today");
}

#[test]
fn moves_to_a_day_go_through_the_plan_other_edits_stay() {
    let snapshot = Snapshot::default();
    let at = memory().at;
    let to_day = Command::MoveTask { id: id(10), planned_on: Some(today()), position: Position::Bottom, at };
    assert_eq!(
        plan_edit(&snapshot, memory(), to_day.clone()),
        Command::EditPlan { occurrence: None, now: memory().at, edit: Box::new(to_day) },
        "sorts among the day's previews"
    );
    let unplan = Command::MoveTask { id: id(10), planned_on: None, position: Position::Bottom, at };
    assert_eq!(plan_edit(&snapshot, memory(), unplan.clone()), unplan);
    let rename = Command::RenameTask { id: id(10), title: "New".into() };
    assert_eq!(plan_edit(&snapshot, memory(), rename.clone()), rename, "no preview: unchanged");
}

#[test]
fn move_to_offers_the_coming_week_and_not_planned() {
    let targets = move_targets(today());
    let labels: Vec<&str> = targets.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(labels[..3], ["Today", "Tomorrow", "Friday, Nov 27"]);
    assert_eq!(targets.len(), 8);
    assert_eq!(targets[1].day_iso.as_deref(), Some("2026-11-26"));
    assert_eq!(targets[7], MoveTarget { label: "Not planned".into(), day_iso: None });
}
