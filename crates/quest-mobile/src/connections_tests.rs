use chrono::{DateTime, Utc};
use quest_store::{AiConnection, ConnectionsError};
use uuid::Uuid;

use super::*;
use crate::test_support::berlin;

fn at(rfc3339: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(rfc3339).unwrap().with_timezone(&Utc)
}

fn connection(name: &str, created: &str, used: Option<&str>) -> AiConnection {
    AiConnection {
        id: Uuid::from_u128(7),
        client_name: name.into(),
        redirect_host: "claude.ai".into(),
        created_at: at(created),
        last_used_at: used.map(at),
        scope: "quest:write".into(),
    }
}

fn now() -> Now {
    // Tuesday, 24 Nov 2026, 10:30 in Berlin.
    berlin("2026-11-24T10:30:00+01:00")
}

fn detail(created: &str, used: Option<&str>) -> String {
    rows(&[connection("Claude", created, used)], now()).remove(0).detail
}

#[test]
fn a_row_names_the_client_its_host_and_when_it_was_used() {
    let row =
        rows(&[connection("  Claude Code ", "2026-11-03T08:00:00Z", Some("2026-11-22T12:00:00Z"))], now()).remove(0);
    assert_eq!(row.id, Uuid::from_u128(7).to_string());
    assert_eq!(row.name, "Claude Code");
    assert_eq!(row.host, "claude.ai");
    assert_eq!(row.detail, "Connected Nov 3 · Last used 2 days ago");
}

#[test]
fn last_use_reads_in_local_calendar_days() {
    assert_eq!(detail("2026-11-03T08:00:00Z", None), "Connected Nov 3 · Not used yet");
    // 00:30 in Berlin is still the 24th, though UTC says the 23rd.
    assert_eq!(detail("2026-11-03T08:00:00Z", Some("2026-11-23T23:30:00Z")), "Connected Nov 3 · Last used today");
    assert_eq!(detail("2026-11-03T08:00:00Z", Some("2026-11-23T22:30:00Z")), "Connected Nov 3 · Last used yesterday");
    assert_eq!(detail("2026-11-03T08:00:00Z", Some("2026-11-18T10:00:00Z")), "Connected Nov 3 · Last used 6 days ago");
    assert_eq!(detail("2026-11-03T08:00:00Z", Some("2026-11-17T10:00:00Z")), "Connected Nov 3 · Last used Nov 17");
    assert_eq!(
        detail("2025-12-30T08:00:00Z", Some("2025-12-31T10:00:00Z")),
        "Connected Dec 30, 2025 · Last used Dec 31, 2025"
    );
    // A clock running behind the server's must not read as "in the future".
    assert_eq!(detail("2026-11-03T08:00:00Z", Some("2026-11-25T10:00:00Z")), "Connected Nov 3 · Last used today");
}

#[test]
fn a_row_says_whether_the_client_may_write() {
    let mut read_only = connection("Claude", "2026-11-03T08:00:00Z", None);
    read_only.scope = "quest:read".into();
    let writer = connection("Claude", "2026-11-03T08:00:00Z", None);
    let labels: Vec<String> = rows(&[read_only, writer], now()).into_iter().map(|r| r.scope).collect();
    assert_eq!(labels, ["Read only", "Read and write"]);
}

#[test]
fn a_blank_name_reads_as_unnamed() {
    assert_eq!(rows(&[connection(" ", "2026-11-03T08:00:00Z", None)], now())[0].name, "Unnamed AI client");
}

#[test]
fn store_errors_become_mobile_errors() {
    assert_eq!(connections_error(ConnectionsError::SignedOut), MobileError::NotSignedIn);
    assert_eq!(connections_error(ConnectionsError::SessionExpired), MobileError::SessionExpired);
    assert_eq!(connections_error(ConnectionsError::NotFound), MobileError::UnknownConnection);
    assert_eq!(
        connections_error(ConnectionsError::Unreachable { message: "timed out".into() }),
        MobileError::ServerUnreachable { detail: "timed out".into() }
    );
    assert_eq!(
        connections_error(ConnectionsError::Server { message: "nope".into() }),
        MobileError::Rejected { reason: "nope".into() }
    );
}
