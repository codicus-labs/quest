use serde_json::json;

use super::*;

fn connection(name: &str, created: &str, used: Option<&str>) -> AiConnection {
    connection_with_scope(name, created, used, "quest:read")
}

fn connection_with_scope(name: &str, created: &str, used: Option<&str>, scope: &str) -> AiConnection {
    serde_json::from_value(json!({
        "id": "0196d1a0-0000-7000-8000-000000000001",
        "client_name": name,
        "redirect_host": "claude.ai",
        "created_at": created,
        "last_used_at": used,
        "scope": scope,
    }))
    .expect("a connection")
}

fn day(raw: &str) -> NaiveDate {
    raw.parse().expect("a date")
}

fn utc_day(at: DateTime<Utc>) -> NaiveDate {
    at.date_naive()
}

#[test]
fn detail_names_host_and_days() {
    let today = day("2026-11-24");
    let fresh = connection("Claude Code", "2026-11-24T12:00:00Z", None);
    assert_eq!(connection_detail(&fresh, today, utc_day), "claude.ai · Read only · Connected today · Not used yet");
    let used = connection("Claude Code", "2026-11-03T12:00:00Z", Some("2026-11-23T12:00:00Z"));
    assert_eq!(
        connection_detail(&used, today, utc_day),
        "claude.ai · Read only · Connected Nov 3 · Last used yesterday"
    );
    let old = connection("Claude Code", "2025-12-30T12:00:00Z", Some("2026-02-01T12:00:00Z"));
    assert_eq!(
        connection_detail(&old, today, utc_day),
        "claude.ai · Read only · Connected Dec 30, 2025 · Last used Feb 1"
    );
}

#[test]
fn a_nameless_client_gets_a_stand_in() {
    assert_eq!(connection_name(&connection("  ", "2026-11-24T12:00:00Z", None)), "Unnamed AI client");
    assert_eq!(connection_name(&connection(" Claude ", "2026-11-24T12:00:00Z", None)), "Claude");
}

#[test]
fn detail_says_what_the_client_may_do() {
    let today = day("2026-11-24");
    let writer = connection_with_scope("Claude", "2026-11-24T12:00:00Z", None, "quest:write");
    assert_eq!(
        connection_detail(&writer, today, utc_day),
        "claude.ai · Read and write · Connected today · Not used yet"
    );
    assert_eq!(scope_label("quest:read"), "Read only");
    assert_eq!(scope_label("offline_access quest:write"), "Read and write");
}
