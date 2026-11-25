# Plan: horizontal scrolling (Round 7)

Measured 2026-11-23 on a MacBook Pro with a 120 Hz ProMotion display (built-in, 3024 × 1964), Rune `3f0d997`.

## Summary

- **Cause of the jank:** mostly the **debug build**. Unoptimised Rune spends about 17 ms on every plan frame, even
  one that doesn't commit, so a debug build can't hold 120 Hz (8.3 ms) or even 60 Hz while the plan scrolls. The
  release build already presented at 120 Hz before this change, with one missed refresh.
- **App side:** `plan_strip_moved` re-ran the view each time the first visible day changed, so fast swipes and
  momentum committed on almost every second frame. It now commits only when the built window of days or the
  month in the header changes. A built window also stays in place while at least 2 built days remain beyond each
  visible edge (hysteresis). Commits over the same swipe dropped from 56 to 8 (release replay) and from 49 to 7
  (runtime test, six weeks).
- **The "hang" in the debug test run** came from Rune's `--native --trace`, not the app: in trace mode an idle
  window keeps redrawing (about 100 fps). In a debug build each traced plan frame costs about 70 ms, so the main
  thread is saturated and input stops arriving. Without `--trace` the same debug binary polls about twice a
  second and draws nothing.

## Method

| | |
| --- | --- |
| Data | `--demo` plus 703 extra tasks on every day from −70 to +70 days (3–7 per day, a third with long, wrapping titles); a throwaway seeding tool on `quest-store` |
| Window | 1100 × 750 pt, `--now 2026-10-15T14:40` |
| Native input | the user's own trackpad: click, ⌘2, 3 hard swipes later with momentum, 3 back, then a slow swipe each way, then ⌘Q. Synthetic CGEvents were not possible: the terminal has no Accessibility permission. |
| Native measurement | `--native --trace` (macOS present times: `present_dt_ms`, `target_ms`, `quality.cpu_ms` per frame) |
| Replay | headless `--scene plan --script` with a 3500 pt swipe, a fling with `vx` 4000, and both again in the other direction (465 scrolling frames, 60 fps virtual clock). Built with `--features rune/frame-profile`; `RUNE_FRAME_LOG` gives CPU, commits and the reason for each commit per frame. |
| Builds | release (`cargo build --release`), debug (`cargo build`), and debug with `--config 'profile.dev.package."*".opt-level=2'` |

"Scrolling frames" are frames in which the board's scroll offset moved. The first frame (the view mounting) is
left out.

## Native trackpad, release build

| | before | after |
| --- | --- | --- |
| Scrolling frames (days travelled) | 882 (≈ 467) | 2081 (≈ 186) |
| Present interval p50 / p95 / max | 8.33 / 8.33 / 16.67 ms | 8.33 / 8.33 / 8.34 ms |
| Missed refreshes while scrolling (> 12.5 ms) | 1 | 0 |
| Skipped drawables (whole run) | 10 | 5 |
| CPU per scrolling frame p50 / p95 | 1.00 / 2.35 ms | 0.97 / 1.21 ms |
| Target → present lag p50 | 16.67 ms | 16.67 ms |

Spikes: in both runs the costliest scrolling frames (4–6 ms CPU) are window steps, which mount about 270–310
elements for a new week of columns, and the frame that first mounts the plan (28–30 ms, on the click). None of
them missed a refresh after the change. The month header has no spike of its own: it costs one ordinary commit
(about 2–3 ms).

The two runs don't share a script (the user swiped by hand), so frame counts and distances differ. The native
trace doesn't record commits, so native commit counts come from the replay.

## Replay (same script for every build)

| Build | Commits while scrolling | CPU / frame p50 / p95 / max | CPU, committing frames p50 / p95 | CPU, other frames p50 / p95 |
| --- | --- | --- | --- | --- |
| Release, before | 56 | 0.84 / 2.00 / 4.04 ms | 1.96 / 3.38 ms | 0.77 / 0.92 ms |
| Release, after | 8 | 0.79 / 0.94 / 4.18 ms | 3.33 / 4.18 ms | 0.79 / 0.93 ms |
| Debug, before | 72 | 18.6 / 28.3 / 53.3 ms | 27.8 / 45.2 ms | 16.7 / 19.8 ms |
| Debug, after | 33 | 17.8 / 25.4 / 51.2 ms | 27.5 / 47.5 ms | 17.2 / 19.0 ms |
| Debug with deps at opt-level 2, after | 33 | 1.14 / 3.16 / 5.87 ms | 3.63 / 5.02 ms | 1.06 / 1.49 ms |

After the change, the release build has 8 commits left: 3 window steps that mount new columns (3.6–4.2 ms
each), and 5 that mount nothing (1.9–3.3 ms): the month changes, plus window steps back onto columns Rune still
holds. In debug builds Rune adds a commit about every 0.25 s while
handlers say `live_only`: its live-only check re-runs the view to compare (25 of the 33). Release builds don't
have that check.

A debug plan frame at rest costs 15.8 ms of CPU headless (release: 0.35 ms) with 2544 layout nodes. That is
Rune's paint and hit-test path, unoptimised, and no app change removes it.

## Idle window (no input, data as above)

| Build | without `--trace` | with `--native --trace` |
| --- | --- | --- |
| Debug | `poll` about 2× a second (41 in 16 s); Rune's debug idle check stays quiet | 4602 frames in 45 s (≈ 100 fps) with 8 wheel events, 100 % CPU; Rune warns "idle window still drawing" |
| Release | not measured cleanly (see below) | not measured cleanly (the run caught 275 wheel events from the user) |

The idle windows opened on top, where the user was working, so pointer and wheel input reached them. A CPU-time
check of release without `--trace` (16–46 % over 8 s) can't be told apart from that input, so it doesn't count
as an idle figure. The app is the same before and after this round (no idle-relevant change).

`sample` of the debug + trace process: 100 % of the main thread is in `Host::redraw`, 61 % of it in
`record_trace_frame` (the element snapshot for each trace line), driven by wake events rather than the app's
`wake_after`. On the plan (debug) a traced frame costs about 70 ms (about 14 fps), and input stops arriving: the
user's debug run shows no horizontal wheel events after the click on Plan. The app's own idle behaviour is
covered by `an_idle_window_re_runs_nothing_while_a_timer_runs`, which checks that Home and Plan, with a timer
running and the clock frozen, commit nothing over 3 s.

## Changes

- `plan_strip.rs`: `Strip::next_window` keeps the built window while `HOLD_DAYS` (2) built days remain beyond
  each visible edge (and a dragged card's day stays built), else takes a fresh week-aligned `window`.
  `Strip::month_at` gives the month the middle of the board shows. Tests: no flip-flopping when wiggling at a
  window step, a dragged card's day stays built, and the month boundary.
- `state/plan_scroll.rs`: `plan_strip_moved` compares the built window and the month, not the first day, and
  says `live_only` otherwise.
- `ui/plan.rs`: the board builds `next_window(built, …)`, and the header reads `month_at`.
- Tests: `a_swipe_re_runs_the_view_only_for_new_weeks_and_months` (at most 9 commits over six weeks; the old
  code made 49), and `an_idle_window_re_runs_nothing_while_a_timer_runs`. The edge auto-scroll tests are
  unchanged and pass.

Evidence: `plan-scroll-fling-strip.png` (a fling from Oct 30 across the month boundary, every 4th frame,
release, dense data). The header switches to November, the columns move as one board, and no column is empty.

## Proposal: optimise dependencies in dev builds (not applied)

```toml
[profile.dev.package."*"]
opt-level = 2
```

The workspace crates stay at opt-level 0 (debuggable); Rune, wgpu, parley and the other dependencies are
optimised.

| | deps at opt-level 0 (now) | deps at opt-level 2 |
| --- | --- | --- |
| Clean `cargo build -p quest-app` | 29 s | 75 s |
| Incremental, quest-app changed | 2 s | 1 s |
| Plan scroll CPU p95 (replay) | 25.4 ms | 3.2 ms |

Recommendation: adopt it. The cost is a slower clean build (once per dependency change); the edit loop stays
the same or gets faster. Without it, a debug build can't scroll the plan smoothly at 120 Hz no matter what the
app does.

## Open

- The native debug comparison is missing: debug at opt-level 0 with `--native --trace` can't be used (see the
  idle section), and without `--trace` there are no frame times. A run of the debug build with deps at
  opt-level 2 is still pending.
- Idle CPU of the release build without `--trace`: measure again with the window away from the user's pointer.
- Screen recording: `screencapture -v` delivers only about 60 fps (p50 16.7 ms between frames) and captures
  everything on screen, so it can't show 120 Hz pacing. That recording was deleted. The headless frame strip
  is the evidence.
- Native commit counts: the native trace doesn't record commits. They come from the replay and the runtime test.
- The trace-mode idle redraw is a Rune question (noted in `RUNE_NOTES.md`).
