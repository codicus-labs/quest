use chrono::{Datelike, Days};
use quest_domain::{Command, EntityId, Frequency, Position, RepeatPin, RepeatRule, Weekdays};
use rune::{Key, Mods};

use super::dnd_tests::{center, drag, rect, settle};
use super::plan_scroll_tests::swipe;
use super::tests::{click, click_beside_title, command, demo_app, press, test_app, type_text};
use crate::clock;
use crate::state::Overlay;

#[test]
fn task_repeat_round_trips_through_the_plan_overview() {
    let (mut rt, _dir) = demo_app();
    let id = EntityId::new();
    rt.state.dispatch(Command::CreateTask {
        id,
        title: "Check the next recording".into(),
        quest_id: None,
        day: None,
        position: Position::Top,
    });
    command(&mut rt, '2');
    rt.update(move |s, _| s.open_task_modal(id));
    click(&mut rt, "task-modal-repeat");
    assert_eq!(rt.state.overlay, Overlay::RepeatEditor);
    click(&mut rt, "repeat-frequency-trigger");
    click(&mut rt, "option1");
    click(&mut rt, "repeat-weekday-Fr");
    click(&mut rt, "repeat-pin-trigger");
    click(&mut rt, "option1");
    click(&mut rt, "repeat-save");

    let today = rt.state.today();
    assert_eq!(rt.state.overlay, Overlay::Task);
    let task = rt.state.snap.task(id).unwrap();
    assert_eq!(task.day, Some(today));
    let rule = task.repeat.unwrap().rule;
    assert_eq!(rule.frequency, Frequency::Week);
    assert_eq!(rule.pin, RepeatPin::Top);
    assert!(rule.weekdays.contains(today.weekday()));
    assert!(rule.weekdays.contains(chrono::Weekday::Fri));
    press(&mut rt, Key::Escape, Mods::default());

    click(&mut rt, "plan-recurring");
    assert_eq!(rt.state.overlay, Overlay::RecurringTasks);
    click(&mut rt, &format!("recurring-{id}"));
    assert_eq!(rt.state.overlay, Overlay::RepeatEditor);
    click(&mut rt, "repeat-cancel");
    assert_eq!(rt.state.overlay, Overlay::RecurringTasks);

    let next = today + Days::new(7);
    rt.state.clock = clock::Clock::Fixed(clock::at_local(next, chrono::NaiveTime::from_hms_opt(14, 0, 0).unwrap()));
    rt.frame(0.0);
    assert_eq!(rt.state.snap.tasks_on(next).filter(|task| task.series == Some(id)).count(), 1);

    click(&mut rt, &format!("recurring-{id}"));
    click(&mut rt, "repeat-stop");
    assert_eq!(rt.state.overlay, Overlay::RecurringTasks);
    assert!(rt.state.snap.task(id).unwrap().repeat.is_none());
    assert_eq!(rt.state.snap.tasks_on(next).filter(|task| task.series == Some(id)).count(), 1);
}

#[test]
fn the_plan_shows_future_occurrences_and_saves_only_the_edited_one() {
    let (mut rt, _dir) = demo_app();
    let today = rt.state.today();
    let tomorrow = today + Days::new(1);
    let later = today + Days::new(14);
    let id = EntityId::new();
    let rule = RepeatRule {
        every: 1,
        frequency: Frequency::Day,
        starts: today,
        ends: Some(later),
        time_zone: chrono_tz::UTC,
        pin: RepeatPin::Top,
        weekdays: Weekdays::default(),
        month_day: None,
    };
    rt.state.dispatch(Command::CreateTask {
        id,
        title: "Rehearse".into(),
        quest_id: None,
        day: Some(today),
        position: Position::Top,
    });
    rt.state.dispatch(Command::SetTaskRepeat { id, rule: Some(rule), now: rt.state.now });
    command(&mut rt, '2');
    let preview = |rt: &rune::Runtime<crate::state::State>, day| {
        rt.state.snap.planned_occurrences(day, rt.state.now).find(|task| task.series == Some(id)).unwrap().id
    };
    let first = preview(&rt, tomorrow);
    let distant = preview(&rt, later);
    rt.settle_all();
    rt.frame(0.0);
    assert!(rt.find(&format!("plan-task-{first}")).is_some());
    click_beside_title(&mut rt, "plan", first);
    assert_eq!(rt.state.overlay, Overlay::Task);
    assert_eq!(rt.state.task_modal.as_ref().unwrap().task, first);
    press(&mut rt, Key::Escape, Mods::default());
    assert!(rt.state.snap.task(first).is_none(), "viewing a preview does not save it");

    click(&mut rt, &format!("plan-title-{first}"));
    assert_eq!(rt.state.task_modal.as_ref().map(|m| m.task), Some(first), "the title opens the dialog");
    rt.update(|s, _| {
        let title = &mut s.task_modal_fields().title;
        title.focused = true;
        title.set_selection(0, title.text.len());
    });
    rt.frame(0.0);
    type_text(&mut rt, "Dress rehearsal");
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.snap.task(first).unwrap().title, "Dress rehearsal");
    assert_eq!(rt.state.snap.task(id).unwrap().title, "Rehearse");

    swipe(&mut rt, 2, 40.0);
    assert!(rt.find(&format!("plan-task-{distant}")).is_some());
    assert!(rt.state.snap.task(distant).is_none(), "scrolling does not save the occurrence");
    assert_eq!(rt.state.snap.tasks.iter().filter(|task| task.series == Some(id)).count(), 1);
    rt.update(|s, _| {
        s.dispatch(Command::SetTaskRepeat { id, rule: Some(RepeatRule { ends: Some(tomorrow), ..rule }), now: s.now })
    });
    assert_eq!(rt.state.snap.task(id).unwrap().repeat.unwrap().rule.ends, Some(tomorrow));
    rt.settle_all();
    rt.frame(0.0);
    assert!(rt.find(&format!("plan-task-{distant}")).is_none());
    assert!(rt.state.snap.task(first).is_some());
    rt.state.clock = clock::Clock::Fixed(clock::at_local(tomorrow, chrono::NaiveTime::from_hms_opt(14, 0, 0).unwrap()));
    rt.frame(0.0);
    assert_eq!(rt.state.snap.tasks_on(tomorrow).filter(|task| task.series == Some(id)).count(), 1);
}

#[test]
fn moving_one_occurrence_leaves_the_series_alone_and_completing_another_brings_it_to_today() {
    let (mut rt, _dir) = test_app(false);
    let today = rt.state.today();
    let tomorrow = today + Days::new(1);
    let next = today + Days::new(2);
    let following = today + Days::new(3);
    let source = EntityId::new();
    let rule = RepeatRule {
        every: 1,
        frequency: Frequency::Day,
        starts: today,
        ends: None,
        time_zone: chrono_tz::UTC,
        pin: RepeatPin::Top,
        weekdays: Weekdays::default(),
        month_day: None,
    };
    rt.state.dispatch(Command::CreateTask {
        id: source,
        title: "Rehearse".into(),
        quest_id: None,
        day: Some(today),
        position: Position::Top,
    });
    rt.state.dispatch(Command::SetTaskRepeat { id: source, rule: Some(rule), now: rt.state.now });
    let preview =
        |day| rt.state.snap.planned_occurrences(day, rt.state.now).find(|task| task.series == Some(source)).unwrap().id;
    let moved = preview(tomorrow);
    let finished = preview(next);
    let other = preview(following);
    command(&mut rt, '2');
    settle(&mut rt);
    let from = center(&rt, &format!("plan-title-{moved}"));
    let [x, y, w, h] = rect(&rt, &format!("plan-task-{finished}"));
    drag(&mut rt, from, (x + w / 2.0, y + h + 12.0));
    settle(&mut rt);
    assert_eq!(rt.state.snap.task(moved).map(|task| task.day), Some(Some(next)), "{:?}", rt.state.notice);
    assert!(rt.find(&format!("plan-task-{finished}")).is_some());

    click(&mut rt, &format!("plan-check-{finished}"));
    assert!(rt.state.snap.task(finished).is_some_and(|task| task.day == Some(today) && task.is_done()));
    assert!(rt.state.snap.task(moved).is_some_and(|task| task.day == Some(next) && !task.is_done()));
    assert!(rt.state.snap.planned_occurrences(following, rt.state.now).any(|task| task.id == other));
    assert_eq!(rt.state.snap.task(source).unwrap().repeat.unwrap().rule, rule);

    click(&mut rt, &format!("plan-check-{finished}"));
    assert!(rt.state.snap.task(finished).is_some_and(|task| task.day == Some(today) && !task.is_done()));
    rt.state.clock = clock::Clock::Fixed(clock::at_local(next, chrono::NaiveTime::from_hms_opt(14, 0, 0).unwrap()));
    rt.frame(0.0);
    assert_eq!(
        rt.state.snap.tasks_on(next).filter(|task| task.series == Some(source)).map(|task| task.id).collect::<Vec<_>>(),
        [moved],
    );
}

#[test]
fn unpicking_the_only_weekday_keeps_it_following_the_start() {
    let (mut rt, _dir) = demo_app();
    let id = EntityId::new();
    rt.state.dispatch(Command::CreateTask {
        id,
        title: "Water".into(),
        quest_id: None,
        day: None,
        position: Position::Top,
    });
    rt.update(move |s, _| s.open_repeat_editor(id));
    rt.update(|s, _| s.repeat_editor.frequency = Frequency::Week);
    let today = rt.state.today();
    rt.update(move |s, _| s.toggle_repeat_weekday(today.weekday()));
    assert_eq!(rt.state.repeat_editor.weekdays, Weekdays::default(), "still the start's weekday, implicitly");
    let later = today + Days::new(2);
    rt.update(move |s, _| s.pick_repeat_start(later));
    assert_eq!(rt.state.repeat_editor.shown_weekdays(today), Weekdays::on(later.weekday()));
}
