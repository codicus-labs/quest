use chrono::{NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

use super::*;
use crate::mcp::error::ToolCode;

fn clock() -> Clock {
    Clock { now: Utc.with_ymd_and_hms(2026, 11, 24, 23, 30, 0).unwrap(), zone: "Europe/Berlin".parse().unwrap() }
}

#[test]
fn today_and_instants_are_in_the_callers_zone() {
    let clock = clock();
    assert_eq!(clock.today(), NaiveDate::from_ymd_opt(2026, 11, 25).unwrap());
    assert_eq!(clock.instant(clock.now), "2026-11-25T00:30:00+01:00");
}

#[test]
fn time_zone_defaults_and_rejects_unknown_names() {
    assert_eq!(time_zone(None, Tz::UTC).unwrap(), Tz::UTC);
    assert_eq!(time_zone(Some(" "), Tz::UTC).unwrap(), Tz::UTC);
    assert_eq!(time_zone(Some("Europe/Berlin"), Tz::UTC).unwrap(), Tz::Europe__Berlin);
    assert_eq!(time_zone(Some("Mars/Base"), Tz::UTC).unwrap_err().code, ToolCode::InvalidInput);
}

#[test]
fn days_parse_iso_or_none() {
    assert_eq!(parse_day_or_none("none", "planned_on").unwrap(), None);
    assert_eq!(parse_day_or_none("2026-11-24", "planned_on").unwrap(), NaiveDate::from_ymd_opt(2026, 11, 24));
    let error = parse_day("24.11.2026", "day").unwrap_err();
    assert!(error.message.contains("ISO date"), "{}", error.message);
}

#[test]
fn ranges_are_bounded() {
    let day = |d| NaiveDate::from_ymd_opt(2026, 11, d).unwrap();
    assert_eq!(day_range(day(1), day(3)).unwrap().len(), 3);
    assert!(day_range(day(3), day(1)).is_err());
    let last = day(1) + chrono::Days::new(61);
    assert_eq!(day_range(day(1), last).unwrap().len(), 62);
    assert!(day_range(day(1), last + chrono::Days::new(1)).is_err());
}

#[test]
fn ids_must_be_uuids() {
    assert!(parse_id("not-an-id", "task_id").unwrap_err().message.contains("task_id"));
    let id = EntityId::new();
    assert_eq!(parse_id(&id.to_string(), "task_id").unwrap(), id);
}

fn workspace(id: EntityId, name: &str, deleted: bool) -> Workspace {
    let deleted_at = deleted.then(|| Utc.with_ymd_and_hms(2026, 11, 1, 8, 0, 0).unwrap());
    Workspace { id, name: name.to_owned(), deleted_at, content: Default::default() }
}

#[test]
fn workspace_defaults_and_resolves_by_id_or_case_insensitive_name() {
    let work = EntityId::new();
    let workspaces = [workspace(EntityId::DEFAULT_WORKSPACE, "Private", false), workspace(work, "Work", false)];
    assert_eq!(resolve_workspace(&workspaces, None).unwrap(), EntityId::DEFAULT_WORKSPACE);
    assert_eq!(resolve_workspace(&workspaces, Some("  ")).unwrap(), EntityId::DEFAULT_WORKSPACE);
    assert_eq!(resolve_workspace(&workspaces, Some(&work.to_string())).unwrap(), work);
    assert_eq!(resolve_workspace(&workspaces, Some(" wORK ")).unwrap(), work);
    let default_id = EntityId::DEFAULT_WORKSPACE.to_string();
    assert_eq!(resolve_workspace(&workspaces, Some(&default_id)).unwrap(), EntityId::DEFAULT_WORKSPACE);
}

#[test]
fn unknown_deleted_and_ambiguous_workspaces_are_refused() {
    let gone = EntityId::new();
    let workspaces = [
        workspace(EntityId::DEFAULT_WORKSPACE, "Default", false),
        workspace(gone, "Old", true),
        workspace(EntityId::new(), "Club", false),
        workspace(EntityId::new(), "club", false),
    ];
    for selector in [EntityId::new().to_string(), gone.to_string(), "Old".into(), "Nope".into(), "CLUB".into()] {
        let error = resolve_workspace(&workspaces, Some(&selector)).unwrap_err();
        assert_eq!(error.code, ToolCode::WorkspaceNotFound, "{selector}");
        assert!(error.message.contains("list_workspaces"), "{selector}: {}", error.message);
    }
}

#[test]
fn workspaces_report_tracked_hours_to_a_tenth() {
    let mut work = workspace(EntityId::new(), "Work", false);
    work.content.tracked_ms = 95 * 60_000;
    let out = workspace_out(&work);
    assert_eq!((out.tracked_hours, out.is_default), (1.6, false));
}
