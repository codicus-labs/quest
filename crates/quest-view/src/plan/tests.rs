use chrono::{Duration, TimeZone};
use quest_domain::{EntityId, Snapshot, Task};

use super::*;

#[test]
fn sections_share_today_and_separate_due_markers_from_rows() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 25).unwrap();
    let now = Utc.from_utc_datetime(&today.and_hms_opt(12, 0, 0).unwrap());
    let offset = |days| today + Duration::days(days);
    let task = |id: u128, plan: Option<i64>, due: Option<i64>, key: &str, completed_at| Task {
        id: EntityId::new(),
        title: id.to_string(),
        quest_id: Some(EntityId::DEFAULT_WORKSPACE),
        planned_on: plan.map(offset),
        due_on: due.map(offset),
        day_order: key.into(),
        sort_key: "V".into(),
        completed_at,
        description: String::new(),
        repeat: None,
        series: None,
    };
    let snapshot = Snapshot {
        tasks: vec![
            task(1, Some(-1), Some(2), "X", None),
            task(2, None, Some(0), "V", None),
            task(3, Some(1), Some(1), "U", None),
            task(4, Some(18), None, "T", None),
            task(5, Some(0), None, "S", Some(now)),
        ],
        ..Snapshot::default()
    };
    let plan = build(&snapshot, today, now, &Utc);
    assert_eq!(plan.days.len(), 14);
    assert_eq!(plan.days[0].tasks.iter().map(Cow::as_ref).collect::<Vec<_>>(), lists::today(&snapshot, today));
    assert_eq!(plan.done_today, [&snapshot.tasks[4]]);
    assert!(plan.days[1].due.is_empty());
    assert_eq!(plan.days[2].due, [&snapshot.tasks[0]]);
    assert_eq!(plan.not_planned, [&snapshot.tasks[1]]);
    assert!(plan.later[0].partial);
    assert_eq!(plan.later[0].days[0].date, offset(18));
    let weeks = weeks(&plan, today);
    assert!(weeks.iter().all(|week| week.len() == 7 && week[0].date.weekday() == chrono::Weekday::Mon));
    assert!(!weeks[0][0].jump_enabled);
    assert!(weeks[0][4].has_tasks);
}
