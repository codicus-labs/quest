use super::*;

use crate::command::{Command, Position};
use crate::test_support::Replica;

fn day(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn at(day: NaiveDate, hour: u32) -> DateTime<Utc> {
    day.and_hms_opt(hour, 0, 0).unwrap().and_utc()
}

fn rule(starts: NaiveDate, frequency: Frequency) -> RepeatRule {
    RepeatRule {
        every: 1,
        frequency,
        starts,
        ends: None,
        time_zone: chrono_tz::Europe::Berlin,
        pin: RepeatPin::None,
        weekdays: Weekdays::default(),
        month_day: None,
    }
}

#[test]
fn month_and_year_rules_clamp_without_losing_the_original_anchor() {
    let monthly = rule(day(2026, 1, 31), Frequency::Month);
    assert_eq!(
        monthly.dates_from(monthly.starts).take(3).collect::<Vec<_>>(),
        [day(2026, 1, 31), day(2026, 2, 28), day(2026, 3, 31)]
    );
    let yearly = rule(day(2024, 2, 29), Frequency::Year);
    assert_eq!(yearly.dates_from(day(2026, 3, 1)).next(), Some(day(2027, 2, 28)));
    assert_eq!(yearly.dates_from(day(2028, 1, 1)).next(), Some(day(2028, 2, 29)));
}

#[test]
fn every_other_week_uses_an_inclusive_end_date() {
    let starts = day(2026, 10, 1);
    let ends = day(2026, 10, 29);
    let weekly = RepeatRule { every: 2, ends: Some(ends), ..rule(starts, Frequency::Week) };
    assert_eq!(weekly.dates_from(starts).collect::<Vec<_>>(), [starts, day(2026, 10, 15), ends]);
    assert_eq!(weekly.dates_from(day(2026, 10, 16)).collect::<Vec<_>>(), [ends]);
}

fn series(replica: &mut Replica, starts: NaiveDate) -> EntityId {
    let id = EntityId::new();
    replica.run(Command::CreateTask {
        id,
        title: "Write".to_owned(),
        quest_id: None,
        planned_on: Some(starts),
        due_on: None,
        position: Position::Bottom,
    });
    replica.run(Command::SetTaskRepeat { id, rule: Some(rule(starts, Frequency::Day)), now: at(starts, 9) });
    id
}

#[test]
fn due_generation_observes_the_series_time_zone_and_preserves_moved_or_deleted_occurrences() {
    let mut replica = Replica::new();
    let starts = day(2026, 7, 2);
    let id = series(&mut replica, starts);
    let next = day(2026, 7, 3);
    let generated = occurrence_id(id, next);

    replica.run(Command::GenerateDueTasks { now: at(starts, 21) });
    assert_eq!(replica.snapshot.tasks.len(), 1);
    replica.run(Command::GenerateDueTasks { now: at(starts, 22) });
    assert_eq!(replica.snapshot.task(generated).unwrap().planned_on, Some(next));
    assert!(!replica.snapshot.task(generated).unwrap().sort_key.is_empty());

    replica.run(Command::MoveTask { id: generated, planned_on: None, position: Position::Top, at: at(next, 9) });
    replica.run(Command::GenerateDueTasks { now: at(next, 9) });
    assert_eq!(replica.snapshot.task(generated).unwrap().planned_on, None);
    replica.run(Command::DeleteTask { id: generated, at: at(next, 9) });
    replica.run(Command::GenerateDueTasks { now: at(next, 10) });
    assert_eq!(replica.snapshot.tasks.len(), 1);
    assert_eq!(replica.snapshot.trash[0].id, generated);
}

#[test]
fn separate_replicas_generate_one_occurrence_and_its_user_edit_takes_precedence() {
    let mut left = Replica::new();
    let starts = day(2026, 11, 21);
    let id = series(&mut left, starts);
    let mut right = Replica::new();
    right.store = left.store.clone();
    right.snapshot = Snapshot::from_store(&right.store, EntityId::DEFAULT_WORKSPACE);
    let next = day(2026, 11, 22);
    let due = Command::GenerateDueTasks { now: at(next, 9) };
    left.run(due.clone());
    right.run(due);
    let generated = occurrence_id(id, next);
    left.run(Command::RenameTask { id: generated, title: "My edit".to_owned() });

    let changes: Vec<_> = right
        .store
        .get(EntityKind::Task, generated)
        .unwrap()
        .iter()
        .map(|(field, (value, _))| crate::FieldChange {
            kind: EntityKind::Task,
            id: generated,
            field: field.clone(),
            value: value.clone(),
            hlc: crate::Hlc { wall_ms: 100, counter: 0, node: crate::NodeId(2) },
        })
        .collect();
    for change in changes {
        left.store.apply(&change);
    }
    let combined = Snapshot::from_store(&left.store, EntityId::DEFAULT_WORKSPACE);
    assert_eq!(combined.tasks.len(), 2);
    assert_eq!(combined.task(generated).unwrap().title, "My edit");
}

#[test]
fn changing_a_schedule_does_not_backfill_the_new_cadence() {
    let mut replica = Replica::new();
    let starts = day(2026, 10, 1);
    let id = series(&mut replica, starts);
    let today = day(2026, 10, 29);
    replica.run(Command::SetTaskRepeat { id, rule: Some(rule(starts, Frequency::Week)), now: at(today, 16) });
    replica.run(Command::GenerateDueTasks { now: at(today, 16) });
    assert_eq!(replica.snapshot.tasks.len(), 2);
    assert!(replica.snapshot.task(occurrence_id(id, today)).is_some());

    replica.run(Command::SetTaskRepeat { id, rule: None, now: at(today, 16) });
    replica.run(Command::GenerateDueTasks { now: at(day(2026, 11, 5), 16) });
    assert_eq!(replica.snapshot.tasks.len(), 2);
}

#[test]
fn the_existing_task_covers_its_day_even_when_the_schedule_starts_earlier() {
    let mut replica = Replica::new();
    let starts = day(2026, 11, 23);
    let assigned = day(2026, 11, 24);
    let id = series(&mut replica, assigned);
    replica.run(Command::SetTaskRepeat { id, rule: None, now: at(starts, 9) });
    replica.run(Command::SetTaskRepeat { id, rule: Some(rule(starts, Frequency::Day)), now: at(starts, 9) });
    replica.run(Command::GenerateDueTasks { now: at(assigned, 9) });
    assert_eq!(replica.snapshot.tasks_on(assigned).count(), 1);
    assert_eq!(replica.snapshot.tasks_on(starts).count(), 1);
}

#[test]
fn a_synced_rule_with_invalid_interval_has_no_dates() {
    let starts = day(2026, 11, 24);
    let invalid = RepeatRule { every: 0, ..rule(starts, Frequency::Day) };
    assert_eq!(invalid.dates_from(starts).next(), None);
}

#[test]
fn selected_weekdays_repeat_together_in_the_same_cadence_week() {
    let starts = day(2026, 10, 1);
    let weekly = RepeatRule {
        every: 2,
        weekdays: Weekdays::on(Weekday::Mon).toggled(Weekday::Thu),
        ..rule(starts, Frequency::Week)
    };
    assert_eq!(
        weekly.dates_from(starts).take(5).collect::<Vec<_>>(),
        [starts, day(2026, 10, 12), day(2026, 10, 15), day(2026, 10, 26), day(2026, 10, 29)]
    );
    assert_eq!(weekly.dates_from(day(2026, 10, 13)).next(), Some(day(2026, 10, 15)));
}

#[test]
fn monthly_rules_can_choose_a_day_number_or_the_last_day() {
    let starts = day(2026, 1, 16);
    let tenth = RepeatRule { month_day: Some(MonthDay::Day(10)), ..rule(starts, Frequency::Month) };
    assert_eq!(
        tenth.dates_from(starts).take(3).collect::<Vec<_>>(),
        [day(2026, 2, 10), day(2026, 3, 10), day(2026, 4, 10)]
    );
    let last = RepeatRule { month_day: Some(MonthDay::Last), ..tenth };
    assert_eq!(
        last.dates_from(starts).take(3).collect::<Vec<_>>(),
        [day(2026, 1, 31), day(2026, 2, 28), day(2026, 3, 31)]
    );
}

#[test]
fn generated_tasks_respect_pin_and_get_the_sources_checklist_unticked_without_a_deadline() {
    let mut replica = Replica::new();
    let starts = day(2026, 11, 23);
    let id = series(&mut replica, starts);
    let next = day(2026, 11, 24);
    let sibling = EntityId::new();
    let bottom = series(&mut replica, starts);
    replica.run(Command::CreateTask {
        id: sibling,
        title: "Existing task".into(),
        quest_id: None,
        planned_on: Some(next),
        due_on: None,
        position: Position::Top,
    });
    let (step, ticked) = (EntityId::new(), EntityId::new());
    for (item, text) in [(step, "Pack"), (ticked, "Go")] {
        replica.run(Command::AddChecklistItem { id: item, task_id: id, text: text.into(), position: Position::Bottom });
    }
    replica.run(Command::SetChecklistItemDone { id: ticked, done: true });
    replica.run(Command::SetTaskDue { id, due_on: Some(starts), today: starts, at: at(starts, 8) });
    for (id, pin) in [(id, RepeatPin::Top), (bottom, RepeatPin::Bottom)] {
        replica.run(Command::SetTaskRepeat {
            id,
            rule: Some(RepeatRule { pin, ..rule(starts, Frequency::Day) }),
            now: at(starts, 9),
        });
    }
    replica.run(Command::GenerateDueTasks { now: at(next, 9) });
    assert_eq!(
        replica.snapshot.tasks_on(next).map(|task| task.id).collect::<Vec<_>>(),
        [occurrence_id(id, next), sibling, occurrence_id(bottom, next)]
    );
    let occurrence = occurrence_id(id, next);
    assert_eq!(replica.snapshot.task(occurrence).unwrap().due_on, None);
    let copied: Vec<(&str, bool)> =
        replica.snapshot.checklist_of(occurrence).map(|item| (item.text.as_str(), item.done)).collect();
    assert_eq!(copied, [("Pack", false), ("Go", false)]);
}

#[test]
fn two_devices_copy_the_same_checklist_offline_and_keep_later_edits() {
    let mut a = Replica::on_node(1);
    let starts = day(2026, 11, 23);
    let id = series(&mut a, starts);
    a.run(Command::AddChecklistItem {
        id: EntityId::new(),
        task_id: id,
        text: "Pack".into(),
        position: Position::Bottom,
    });
    let mut b = Replica::on_node(2);
    b.merge(&a);
    let next = day(2026, 11, 24);
    a.run(Command::GenerateDueTasks { now: at(next, 9) });
    let item = a.snapshot.checklist_of(occurrence_id(id, next)).next().unwrap().id;
    a.run(Command::SetChecklistItemDone { id: item, done: true });
    b.run(Command::GenerateDueTasks { now: at(next, 10) });
    b.run(Command::GenerateDueTasks { now: at(next, 11) });
    a.merge(&b);
    b.merge(&a);
    assert_eq!(a.snapshot.checklist, b.snapshot.checklist);
    assert_eq!(a.snapshot.checklist_of(occurrence_id(id, next)).count(), 1);
    assert!(a.snapshot.checklist_item(item).unwrap().done, "the tick outlives the other device's copy");
}

#[test]
fn preview_edits_use_the_occurrence_id_and_a_deleted_preview_does_not_return() {
    let mut replica = Replica::new();
    let first = day(2026, 11, 23);
    let next = first.succ_opt().unwrap();
    let now = at(first, 9);
    let source = series(&mut replica, first);
    let id = occurrence_id(source, next);
    assert_eq!(replica.snapshot.planned_occurrences(next, now).next().unwrap().id, id);
    assert_eq!(replica.snapshot.planned_occurrences(first, now).count(), 0);
    assert!(replica.snapshot.task(id).is_none());
    let edit = |edit| Command::EditPlan { occurrence: Some((source, next)), now, edit: Box::new(edit) };
    let invalid = edit(Command::RenameTask { id, title: "".into() });
    assert_eq!(plan(&invalid, &replica.snapshot), Err(CommandError::EmptyTitle));

    replica.run(edit(Command::DeleteTask { id, at: now }));
    assert!(replica.snapshot.task(id).is_none());
    assert_eq!(replica.snapshot.trash[0].id, id);
    assert_eq!(replica.snapshot.planned_occurrences(next, now).count(), 0);
    replica.run(Command::GenerateDueTasks { now: at(next, 9) });
    assert!(replica.snapshot.task(id).is_none());
}
