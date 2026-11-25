use std::sync::Arc;

use super::*;
use crate::core::ChangeListener;
use crate::task_types::{RepeatFrequency, RepeatPinChoice};
use crate::test_support::moment;
use crate::types::{AddPosition, HomeData, HomeQuery, TaskSection};

struct Quiet;

impl ChangeListener for Quiet {
    fn on_change(&self) {}
}

fn open() -> (tempfile::TempDir, Arc<QuestCore>) {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    (dir, core)
}

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn now() -> Moment {
    moment("2026-11-25T10:00:00+01:00")
}

fn later(seconds: i64) -> Moment {
    Moment { epoch_ms: now().epoch_ms + seconds * 1000, ..now() }
}

fn home(core: &QuestCore) -> HomeData {
    core.home(now(), HomeQuery::default()).unwrap()
}

fn add(core: &QuestCore, title: &str) -> String {
    core.add_task(title.into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap()
}

#[test]
fn a_delete_can_be_undone_from_the_snackbar() {
    let (_dir, core) = open();
    let id = add(&core, "Draft");
    core.delete_task(id.clone(), now()).unwrap();
    let data = home(&core);
    assert!(data.other.is_empty());
    assert_eq!(data.toast.map(|t| t.text), Some("Deleted “Draft”".into()));
    core.undo(now()).unwrap();
    let data = home(&core);
    assert_eq!(data.other[0].id, id);
    assert_eq!(data.toast, None);
}

#[test]
fn a_swipe_completion_can_be_undone_and_its_toast_expires_on_tick() {
    let (_dir, core) = open();
    let id = add(&core, "Draft");
    core.toggle_task(id.clone(), now()).unwrap();
    let toast = home(&core).toast.unwrap();
    assert_eq!(toast.text, "Completed “Draft”");
    assert_eq!(core.next_wake_ms(now()).unwrap(), toast.expires_at_ms + 5, "wakes when the snackbar goes");
    core.undo(now()).unwrap();
    assert!(!home(&core).other[0].done);

    core.toggle_task(id, now()).unwrap();
    core.tick(later(6)).unwrap();
    assert_eq!(home(&core).toast, None);
    assert!(home(&core).other[0].done);
}

#[test]
fn renames_skip_empty_titles() {
    let (_dir, core) = open();
    let id = add(&core, "Draft");
    core.rename_task(id.clone(), "  ".into()).unwrap();
    assert_eq!(home(&core).other[0].title, "Draft");
    core.rename_task(id, "Final\ndraft".into()).unwrap();
    assert_eq!(home(&core).other[0].title, "Final draft");
}

#[test]
fn starting_a_timer_stops_the_other_one() {
    let (_dir, core) = open();
    let (first, second) = (add(&core, "First"), add(&core, "Second"));
    core.start_timer(first, now()).unwrap();
    core.start_timer(second.clone(), later(60)).unwrap();
    let data = core.home(later(120), HomeQuery::default()).unwrap();
    let running: Vec<_> = data.other.iter().map(|t| (t.timer_running, t.tracked.clone())).collect();
    assert_eq!(running, [(false, Some("01:00".into())), (true, Some("01:00".into()))]);
    core.stop_timer(later(120)).unwrap();
    assert!(!core.home(later(120), HomeQuery::default()).unwrap().other[1].timer_running);
    core.start_timer(second.clone(), later(120)).unwrap();
    core.start_timer(second.clone(), later(130)).unwrap();
    let data = core.home(later(180), HomeQuery::default()).unwrap();
    assert_eq!(data.other[1].tracked.as_deref(), Some("02:00"), "a second start on the running task does nothing");
    let entries = core.task_detail(second.clone(), later(180)).unwrap().logged.len();
    assert_eq!(entries, 2, "no split session");
    core.toggle_task(second.clone(), now()).unwrap();
    assert_eq!(core.start_timer(second.clone(), now()), Err(MobileError::TaskIsDone { id: second }));
}

#[test]
fn the_sheet_edits_description_quest_and_day() {
    let (_dir, core) = open();
    let id = add(&core, "Draft");
    core.set_task_description(id.clone(), "Notes\nmore".into()).unwrap();
    core.set_task_day(id.clone(), Some("2026-11-26".into()), now()).unwrap();
    let detail = core.task_detail(id.clone(), now()).unwrap();
    assert_eq!(detail.description, "Notes\nmore");
    assert_eq!(detail.date.map(|d| d.label), Some("Nov 26, 2026".into()));
    assert!(home(&core).other.is_empty(), "moved to tomorrow");
    core.set_task_day(id.clone(), None, now()).unwrap();
    assert_eq!(core.task_detail(id.clone(), now()).unwrap().date, None);
    let bad = core.set_task_day(id.clone(), Some("tomorrow".into()), now());
    assert_eq!(bad, Err(MobileError::InvalidDate { value: "tomorrow".into() }));
    core.set_task_quest(id.clone(), None).unwrap();
    core.delete_task(id.clone(), now()).unwrap();
    assert_eq!(core.task_detail(id.clone(), now()), Err(MobileError::UnknownTask { id }));
}

#[test]
fn repeating_creates_occurrences_that_edit_their_series() {
    let (_dir, core) = open();
    let id = add(&core, "Stretch");
    let draft = core.repeat_draft(id.clone(), now()).unwrap();
    assert_eq!(
        (draft.frequency, draft.pin, draft.starts.as_str()),
        (RepeatFrequency::Day, RepeatPinChoice::None, "2026-11-25")
    );
    let wrong = RepeatDraft { every: "0".into(), ..draft.clone() };
    let refused = core.set_task_repeat(id.clone(), wrong, now());
    assert_eq!(refused, Err(MobileError::InvalidRepeat { reason: "Enter an interval from 1 to 999.".into() }));

    core.set_task_repeat(id.clone(), RepeatDraft { every: "2".into(), ..draft }, now()).unwrap();
    assert_eq!(core.task_detail(id.clone(), now()).unwrap().repeat_label, "Every 2 days");
    let friday = moment("2026-11-27T08:00:00+01:00");
    core.tick(friday.clone()).unwrap();
    let occurrence = core.home(friday.clone(), HomeQuery::default()).unwrap().other.remove(0);
    assert_ne!(occurrence.id, id);
    let detail = core.task_detail(occurrence.id.clone(), friday.clone()).unwrap();
    assert_eq!((detail.repeat_label.as_str(), detail.repeats), ("Every 2 days", true));
    assert_eq!(core.repeat_draft(occurrence.id.clone(), friday.clone()).unwrap().every, "2");

    core.clear_task_repeat(occurrence.id.clone(), friday.clone()).unwrap();
    assert_eq!(core.task_detail(id.clone(), friday.clone()).unwrap().repeat_label, "Does not repeat");
    assert_eq!(core.clear_task_repeat(id, friday), Ok(()), "stopping what does not repeat is no error");
}

#[test]
fn the_day_grid_shows_the_running_timer() {
    let (_dir, core) = open();
    let id = add(&core, "Draft");
    core.start_timer(id, now()).unwrap();
    let grid = core.day_grid("2026-11-25".into(), later(30 * 60)).unwrap();
    let block = &grid.blocks[0];
    assert_eq!((block.start_minute, block.end_minute, block.title.as_str()), (600, 630, "Draft"));
    assert_eq!(grid.now_minute, Some(630.0));
    assert!(matches!(core.day_grid("25.11.".into(), now()), Err(MobileError::InvalidDate { .. })));
}

#[test]
fn quest_work_gets_the_filter_or_pick() {
    let (_dir, core) = open();
    let (book, garden) = (EntityId::new(), EntityId::new());
    for (id, title) in [(book, "Book"), (garden, "Garden")] {
        let create = Command::CreateQuest { id, title: title.into(), color: quest_domain::QuestColor::Blue };
        core.dispatch(create).unwrap();
    }
    let query = HomeQuery { quest_filter: None, quest_pick: Some(garden.to_string()) };
    core.add_task("Weed".into(), TaskSection::QuestWork, AddPosition::Top, query, now()).unwrap();
    let filtered = HomeQuery { quest_filter: Some(book.to_string()), quest_pick: Some(garden.to_string()) };
    core.add_task("Write".into(), TaskSection::QuestWork, AddPosition::Top, filtered.clone(), now()).unwrap();
    let data = core.home(now(), HomeQuery::default()).unwrap();
    let work: Vec<_> =
        data.quest_work.iter().map(|t| (t.title.as_str(), t.quest.as_ref().unwrap().name.as_str())).collect();
    assert_eq!(work, [("Write", "Book"), ("Weed", "Garden")]);
    assert_eq!(data.add_quest.map(|q| q.name), Some("Book".into()), "the quest used last");
    let titles: Vec<_> = core.home(now(), filtered).unwrap().quest_work.into_iter().map(|t| t.title).collect();
    assert_eq!(titles, ["Write"]);
}

#[test]
fn the_repeat_dialog_checks_live() {
    let draft = RepeatDraft {
        every: "1".into(),
        frequency: RepeatFrequency::Week,
        pin: RepeatPinChoice::None,
        weekdays: Vec::new(),
        last_day: false,
        month_day: String::new(),
        starts: "2026-11-25".into(),
        ends_on_date: false,
        ends: String::new(),
        time_zone: "Mars".into(),
    };
    let checked = check_repeat(draft.clone(), now()).unwrap();
    assert_eq!(checked.error.as_deref(), Some("Enter a timezone such as Europe/Berlin or UTC."));
    assert_eq!(checked.summary, "Every week");
    let toggled = toggle_repeat_weekday(draft, RepeatWeekday::Mon, now()).unwrap();
    assert_eq!(toggled.weekdays, [RepeatWeekday::Mon, RepeatWeekday::Wed]);
}
