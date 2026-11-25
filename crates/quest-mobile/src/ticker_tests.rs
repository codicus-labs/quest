use std::sync::Arc;

use chrono::{DateTime, Utc};
use quest_domain::Snapshot;

use super::*;
use crate::types::Moment;

fn utc(rfc3339: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(rfc3339).unwrap().with_timezone(&Utc)
}

/// A wake time: the boundary plus the slack.
fn wake(rfc3339: &str) -> DateTime<Utc> {
    utc(rfc3339) + WAKE_SLACK
}

fn now(rfc3339: &str, zone: &str) -> Now {
    Now::parse(&Moment { epoch_ms: utc(rfc3339).timestamp_millis(), time_zone: zone.into() }).unwrap()
}

fn berlin(rfc3339: &str) -> Now {
    now(rfc3339, "Europe/Berlin")
}

#[test]
fn wakes_at_the_next_whole_minute_without_a_running_timer() {
    assert_eq!(next_wake(berlin("2026-11-25T10:00:20.250+01:00"), false, None), wake("2026-11-25T10:01:00+01:00"));
    assert_eq!(next_wake(berlin("2026-11-25T10:00:00+01:00"), false, None), wake("2026-11-25T10:01:00+01:00"));
}

#[test]
fn wakes_at_the_next_whole_second_while_a_timer_runs() {
    assert_eq!(next_wake(berlin("2026-11-25T10:00:20.250+01:00"), true, None), wake("2026-11-25T10:00:21+01:00"));
    assert_eq!(next_wake(berlin("2026-11-25T10:00:20+01:00"), true, None), wake("2026-11-25T10:00:21+01:00"));
}

#[test]
fn wakes_at_midnight_in_the_callers_zone() {
    assert_eq!(next_wake(berlin("2026-11-25T23:59:30+01:00"), false, None), wake("2026-11-26T00:00:00+01:00"));
    // Liberia ran on UTC-0:44:30 until 1972: midnight falls inside a UTC minute and still wakes.
    let monrovia = now("1960-01-02T00:44:10Z", "Africa/Monrovia");
    assert_eq!(next_wake(monrovia, false, None), wake("1960-01-02T00:44:30Z"));
}

#[test]
fn wakes_across_a_dst_change() {
    // Berlin springs from 02:00 CET to 03:00 CEST.
    assert_eq!(next_wake(berlin("2026-03-29T01:59:30+01:00"), false, None), wake("2026-03-29T03:00:00+02:00"));
    // Santiago skips midnight (Saturday 24:00 -04 becomes Sunday 01:00 -03): the day starts at the jump.
    let santiago = now("2026-09-05T23:59:30-04:00", "America/Santiago");
    assert_eq!(next_wake(santiago, false, None), wake("2026-09-06T01:00:00-03:00"));
    assert_eq!(santiago.midnight(santiago.today() + Days::new(1)), utc("2026-09-06T04:00:00Z"));
}

#[test]
fn generates_on_the_first_tick_then_once_per_minute() {
    let snapshot = Arc::new(Snapshot::default());
    let mut memory = TickMemory::default();
    let first = berlin("2026-11-25T10:00:20+01:00");
    assert!(memory.should_generate(&snapshot, first));

    memory.generated(snapshot.clone(), first);
    assert!(!memory.should_generate(&snapshot, berlin("2026-11-25T10:00:59+01:00")));
    assert!(memory.should_generate(&snapshot, berlin("2026-11-25T10:01:00+01:00")));
}

#[test]
fn generates_again_when_the_data_changed() {
    let mut memory = TickMemory::default();
    let at = berlin("2026-11-25T10:00:20+01:00");
    memory.generated(Arc::new(Snapshot::default()), at);
    assert!(memory.should_generate(&Arc::new(Snapshot::default()), at));
}

#[test]
fn generates_at_midnight_even_within_the_same_utc_minute() {
    let snapshot = Arc::new(Snapshot::default());
    let mut memory = TickMemory::default();
    memory.generated(snapshot.clone(), now("1960-01-02T00:44:10Z", "Africa/Monrovia"));
    assert!(memory.should_generate(&snapshot, now("1960-01-02T00:44:30Z", "Africa/Monrovia")));
}

#[test]
fn a_postponed_generation_runs_on_the_next_tick() {
    let snapshot = Arc::new(Snapshot::default());
    let mut memory = TickMemory::default();
    let at = berlin("2026-11-25T10:00:20+01:00");
    memory.generated(snapshot.clone(), at);
    memory.postpone();
    assert!(memory.should_generate(&snapshot, at));
}

#[test]
fn an_undo_snackbar_that_goes_sooner_wakes_sooner() {
    let now = berlin("2026-11-25T10:00:20+01:00");
    let toast = Some(wake("2026-11-25T10:00:26+01:00") - WAKE_SLACK);
    assert_eq!(next_wake(now, false, toast), wake("2026-11-25T10:00:26+01:00"));
    assert_eq!(next_wake(now, true, toast), wake("2026-11-25T10:00:21+01:00"));
    let gone = Some(now.at - chrono::TimeDelta::seconds(1));
    assert_eq!(next_wake(now, false, gone), wake("2026-11-25T10:01:00+01:00"), "a past expiry does not wake");
}

#[test]
fn the_next_day_starts_at_local_midnight_or_at_the_jump() {
    assert_eq!(next_day(now("2026-11-25T09:00:00Z", "Europe/Berlin")), wake("2026-11-25T23:00:00Z"));
    assert_eq!(next_day(now("2026-11-25T23:30:00Z", "Europe/Berlin")), wake("2026-11-26T23:00:00Z"));
    // Santiago skips Saturday's midnight: Sunday starts at 01:00 -03.
    assert_eq!(next_day(now("2026-09-05T20:00:00Z", "America/Santiago")), wake("2026-09-06T04:00:00Z"));
}
