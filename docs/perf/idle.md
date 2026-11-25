# Idle drawing (Round 9)

Measured 2026-11-23 on a MacBook Pro (120 Hz ProMotion display), Rune `3f0d997`, debug build (dependencies at
`opt-level = 2`), no `--trace`.

## Summary

- **Trigger:** `rune: idle window "Quest" still drawing … (120 frames in 1.2 s …, woken by a wake_after loop)`
  in an interactive debug session (`--demo`, real clock, no trace). The user no longer knows what came just
  before it. The window was used before the warning (six top-bar clicks).
- **Reproduced cause:** the Plan edge scroll. A card dragged and held in the board's edge zone asked for a
  16 ms `wake_after` on every poll, even when the strip could not move: at the strip's end, or so close to the
  zone's inner edge that the step is below one point. A held drag sends no input, and nothing on screen changes,
  so it matches the warning exactly: **515 wakes in 5 s (about 100 per second)**.
- **Fixes:**
  - Edge scroll (`state/plan_scroll.rs`): `EdgeScroll::{Off, Ticking, Stalled}`. A tick that moves nothing
    stops the ticks (`Stalled`), and only a pointer move starts them again. After the fix: 1 wake in 5 s.
  - Timer tick and now-line (`State::refresh`): the app used to wake 1 s or 30 s after each poll. It now wakes
    when the shown second (running timer) or minute (now-line, repeats, the date) turns
    (`clock::next_whole`).
  - Undo toast: the app wakes 5 ms after the expiry (`clock::wake_for`), so an early poll can't keep asking
    for a wake that is almost due.
- **Not reproduced without input:** each view (Home, Plan, Calendar, Focus, Quests, Review) in a real window,
  with and without a running timer. Neither loop showed up in a headless run with a moving clock or in a
  realtime script (tab changes) either.

## Measurements

The headless runtime with a moving clock (`src/ui/idle_tests.rs`): between frames the app clock jumps to the next
`Runtime::wake_deadline`. While something animates, frames come at 120 Hz and don't count as wakes. 10 s each.

| Case | Before: wakes / commits | After: wakes / commits |
| --- | --- | --- |
| Home, timer running | 10 / 10 | 9 / 9 |
| Plan, timer running | 20 / 10 | 19 / 10 |
| Home or Plan, no timer | 1 / 0 | 0 / 0 |
| Undo toast, until it expires | 4 (plus 36 exit-animation frames) / 1 | 1 / 1 (plus the exit animation) |
| Drag held in the Plan edge zone, strip can't move (5 s) | **515** / 0 | **1** / 0 |

On Plan with a running timer, Rune adds a second frame about 33 ms after each tick's commit. That frame
comes from Rune, not from the app's `wake_after`.

A real window (`RUNE_FRAME_LOG`, `--features rune/frame-profile`, fresh `--demo` directory, 12 s with no input):

| | Frames in 12 s | App wakes |
| --- | --- | --- |
| Before, Home (timer running) | 13 | `state.rs:457 1000ms` each poll |
| Before, per view (Home / Plan / Calendar / Focus / Quests / Review) | 34 / 9 / 6 / 10 / 14 / 5 | 1000 ms |
| After, Home | 2 (display link failed: `-6661`; the display was probably off, so the window drew almost nothing) | `state.rs:458 721ms`, the next whole second |

No `idle window still drawing` warning came up in any real-window run, before or after. The interactive case
(a held edge drag) is covered by the test above.

## If the warning shows up again

Rune's warning doesn't say which `wake_after` it means. A frame log does: build with Rune's `frame-profile`
feature, which needs no change to the manifest, and log each frame:

```sh
RUNE_FRAME_LOG=/tmp/quest-frames.jsonl cargo run -p quest-app --features rune/frame-profile -- --demo --data-dir /tmp/quest-a
# after the warning:
python3 -c 'import json,collections; c=collections.Counter(w.rsplit(" ", 1)[0] for l in open("/tmp/quest-frames.jsonl") for w in json.loads(l)["wakers"]); print(c.most_common(5))'
```

Each line has `wakers`: every `Tx::wake_after` call site (`file:line duration`) since the last frame. `next`
gives the reason for the next frame (`live`, `input`, …).
