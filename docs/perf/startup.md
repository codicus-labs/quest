# Startup time (Round 9, measured only)

Measured 2026-11-23 on a MacBook Pro, Rune `3f0d997`. Starting point: `rune: first frame took 1473 ms (setup 1288 ms,
…, window and GPU 181 ms); the slowest is setup in main before run` (debug, `cargo run … --demo --data-dir
/tmp/quest-a`).

## What Rune's "setup" counts

`setup` is **the process's age when `App::run` starts** (`tooling/perf_warn/launch.rs`). That includes everything
before `main` too: `exec`, dyld and macOS's check of the binary. Rune warns only when setup + init + first view
take more than 100 ms. It warns only in debug builds.

## Method

- Temporary `Instant` marks in `main` behind an env flag (since removed): `prewarm_gpu`, `cli::parse`, time
  zone and `AppSpec`, `App::new`, `Store::open`, `seed_demo`, `State::new` (in `init`). The time before `main`
  is a wall-clock stamp in the launching shell against `SystemTime::now()` at the top of `main`. That includes
  about 10 ms to start `python3`.
- Each build was copied to a new file before its first run (`new binary`), as right after `cargo build`. Then
  it ran again with the same data directory (`existing`) and with a new one (`fresh`). Three rounds each; the
  table gives the range.

## Results (ms)

| Build, case | Before `main` | `main` to `run`: total | `Store::open` | `seed_demo` | Rune `setup` (warning) |
| --- | --- | --- | --- | --- | --- |
| debug, new binary, fresh dir | **385–515** | 19–20 | 6.5–7.2 | 7.1–7.2 | 402–532 |
| debug, warm, existing dir | 13–14 (≈ python) | 4.0–4.6 | 2.7–3.3 | < 0.1 | no warning (< 100) |
| debug, warm, fresh dir | 13–14 | 9.2–9.6 | 3.0–3.1 | 4.8–5.1 | no warning |
| release, new binary, fresh dir | **313–504** | 11–14 | 5.9–7.2 | 2.0–2.1 | (release doesn't warn) |
| release, warm, existing dir | 13 | 3.5–3.6 | 2.2–2.3 | < 0.1 | — |
| release, warm, fresh dir | 11–13 | 4.8–5.5 | 2.7–3.0 | 1.0–1.2 | — |

Also measured: `prewarm_gpu` < 0.3 ms (it starts a thread), `cli::parse` < 0.6 ms, time zone and `AppSpec`
0.8–2.8 ms, `App::new` 0.1–0.2 ms, `State::new` 0.1 ms. Rune's `window and GPU` until the first frame: 144–314 ms
(debug, not split further).

## Findings

- **The app's own work before `run` costs 4–25 ms**, debug or release. Opening SQLite (including migrations) and
  seeding the demo are the largest parts, each under 10 ms.
- **The warned "setup" is almost all the time before `main`**, and only on the first start of a newly
  written binary (300–530 ms, release as well). A second start of the same file takes about 13 ms before
  `main`. That points to macOS checking a new executable (code signature / syspolicy) on its first exec. The
  binaries are large (debug 62 MB, release 43 MB). The 1288 ms in the original log came from the first start
  after a build (`cargo run`).
- On a warm start Rune doesn't warn at all, because setup is under 100 ms.
- **`cargo run` adds its own time.** On Unix, cargo starts the program with `exec`, so the program keeps
  cargo's PID, and Rune takes the process start from `proc_pidinfo`. Cargo's start and its up-to-date check
  then count as `setup`: with the same binary, already started once, `cargo run` showed `setup 1043 ms`, and a
  direct start showed no warning (< 100 ms). Right after a rebuild under load it showed `setup 8222 ms`. The
  1288 ms in the original log (`Finished … in 0.54s`) fit this.

## Recommendation (not implemented)

| Candidate | Expected gain | Assessment |
| --- | --- | --- |
| Move `Store::open` (+ `seed_demo`) to `on_ready` or a background thread | 4–20 ms before the first frame | Little gain; the first frame would need an empty or loading state, and headless `--snap` needs data in the first frame (RUNE_NOTES). **Not worth it.** |
| Smaller binary for the first exec (`strip`, `split-debuginfo = "unpacked"` in the dev profile, fewer features) | Part of the 300–500 ms on the first start after a build | Worth trying; check the effect on the first exec with the same method. |
| Production: a signed, notarised bundle | The first-start check happens once per installation | Matters only for distribution, not for development. |
| Rune: `window and GPU` (144–314 ms) | — | Rune's own time; `prewarm_gpu` already runs first in `main`. |

To read Rune's number fairly, start `target/debug/quest-app` directly, not through `cargo run`. The warning "start slow work from `on_ready`" doesn't apply to this app: the slow part is not app code.
