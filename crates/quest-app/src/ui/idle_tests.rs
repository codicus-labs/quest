//! An idle window on a clock that moves: it wakes only when something on screen changes (a
//! running timer each second, the now-line each minute, the undo toast when it goes), as a
//! native window that sleeps until a `wake_after` deadline.

use chrono::Duration;
use rune::Runtime;

use super::dnd_tests::{settle, task};
use super::tests::{command, demo_app};
use crate::clock::Clock;
use crate::state::State;
use crate::undo::Deletable;

/// What `seconds` of an idle native window cost. Between frames the app's clock moves as far
/// as Rune's: to the next wake asked for, or a 120 Hz frame while something animates.
struct Idle {
    /// Frames a wake asked for (not motion).
    woken: u32,
    commits: u64,
}

fn idle(rt: &mut Runtime<State>, seconds: f64) -> Idle {
    let Clock::Fixed(start) = rt.state.clock else { panic!("tests run on a fixed clock") };
    let commits = rt.stats.commits;
    let (mut elapsed, mut woken, mut animating) = (0.0, 0, false);
    let mut clock = rune_clock(rt);
    while seconds - elapsed > 1e-6 {
        let wake = rt.wake_deadline().map_or(f64::INFINITY, |at| (at - clock).max(1.0 / 120.0));
        if !animating && wake > seconds - elapsed {
            break; // Nothing more is due within `seconds`.
        }
        let step = if animating { 1.0 / 120.0 } else { wake };
        woken += u32::from(!animating);
        elapsed += step;
        clock += step;
        rt.state.clock = Clock::Fixed(start + Duration::microseconds((elapsed * 1e6) as i64));
        animating = rt.frame(step);
    }
    Idle { woken, commits: rt.stats.commits - commits }
}

/// The clock `poll` and `wake_after` count on.
fn rune_clock(rt: &mut Runtime<State>) -> f64 {
    let mut clock = 0.0;
    rt.update_if(|_, tx| {
        clock = tx.clock();
        false
    });
    clock
}

/// Settles what the last step started; the live-only check's own view runs aren't counted.
fn quiet(rt: &mut Runtime<State>) {
    rt.check_live_only(false);
    settle(rt);
    idle(rt, 2.0);
}

fn stop_the_timer(rt: &mut Runtime<State>) {
    let running = rt.state.snap.running_entry().and_then(|e| e.task_id).expect("the demo has a running timer");
    rt.update(|s, _| s.toggle_focus(running));
}

#[test]
fn a_running_timer_wakes_home_and_plan_about_once_a_second() {
    let (mut rt, _dir) = demo_app();
    for view in ['1', '2'] {
        command(&mut rt, view);
        quiet(&mut rt);
        let ten = idle(&mut rt, 10.0);
        assert!(ten.commits <= 10, "⌘{view}: {} commits in 10 s", ten.commits);
        // Rune adds a frame after some commits; a loop would be hundreds.
        assert!(ten.woken <= 21, "⌘{view}: {} wakes in 10 s", ten.woken);
    }
}

#[test]
fn without_a_timer_home_and_plan_wake_only_for_the_minute() {
    let (mut rt, _dir) = demo_app();
    stop_the_timer(&mut rt);
    for view in ['1', '2'] {
        command(&mut rt, view);
        quiet(&mut rt);
        let ten = idle(&mut rt, 10.0);
        assert!(ten.commits <= 1, "⌘{view}: {} commits in 10 s", ten.commits);
        assert!(ten.woken <= 3, "⌘{view}: {} wakes in 10 s", ten.woken);
    }
}

#[test]
fn the_undo_toast_goes_once_and_the_window_rests_again() {
    let (mut rt, _dir) = demo_app();
    stop_the_timer(&mut rt);
    command(&mut rt, '2');
    quiet(&mut rt);
    let essay = task(&rt, "Draft this week’s essay").id;
    rt.update(|s, _| s.delete(Deletable::Task(essay)));
    settle(&mut rt);
    assert!(rt.state.undo.shown.is_some(), "the toast shows");
    let ten = idle(&mut rt, 10.0);
    assert!(rt.state.undo.shown.is_none(), "and goes by itself");
    assert!(ten.woken <= 6, "{} wakes in 10 s", ten.woken);
    assert!(ten.commits <= 2, "{} commits in 10 s", ten.commits);
}
