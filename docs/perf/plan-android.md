# Plan on Android: scrolling and dragging (Phase 3)

Measured 2026-11-24 on the emulator `quest-api36` (Android 16, arm64-v8a, 1080 × 2400, 60 Hz) with the
**debug** build on a MacBook (Apple silicon). Emulator numbers are a rough upper bound; a release build on a
device will do better. Same method as for Home in Phase 1, so the numbers compare.

## Method

```sh
adb shell dumpsys gfxinfo app.quest.android reset
# scripted gestures (below)
adb shell dumpsys gfxinfo app.quest.android
```

- **Plan list (phone):** 8 swipes up and 12 down, `input swipe 540 1800 540 600 250` and back. The scroll runs past
  the built window, so new windows are built (`plan_window`) during the run.
- **Board (tablet, `wm size 2560x1600`, `wm density 320`):** 8 swipes left, 10 right,
  `input swipe 2200 900 600 900 250` and back (snap fling between columns).
- **Drag (phone):** 3 × (drag a card 600 px down and drop, which moved it to the next day, then drag another 300 px up and drop),
  `input motionevent` with a long press of 0.8 s; each drop writes through the core and re-reads the plan.
- **Home (phone, comparison):** 8 × swipe up and down as above; Home's list is short and barely scrolls.

## Results

| Run | Frames | Janky | p50 | p90 | p95 | p99 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Plan list, scroll | 323 | 15 (4.6 %) | 17 ms | 20 ms | 21 ms | 32 ms |
| Board, sideways | 293 | 19 (6.5 %) | 17 ms | 19 ms | 20 ms | 32 ms |
| Drag and drop | 280 | 28 (10.0 %) | 17 ms | 21 ms | 23 ms | 34 ms |
| Home, scroll (comparison) | 283 | 3 (1.1 %) | 17 ms | 20 ms | 20 ms | 40 ms |

“Slow UI thread” was 3 for the list and 0 for the board; the rest of the missed deadlines are render and GPU
time in the emulator (`50th gpu percentile` 13 ms).

## Reading

- Scrolling and window building hold the frame rate most of the time. The p99 spikes (~32 ms, one missed frame)
  fall where a new window arrives and the list composes days it has not shown yet.
- Drag is the heaviest: each drop changes the plan, the core rebuilds it, and the list recomposes the moved
  rows. While dragging, the lifted card moves through a graphics-layer translation and the drop line through
  `derivedStateOf` per row, so the finger does not recompose the list.
- Not measured: release build, a physical device, JankStats in the app.
