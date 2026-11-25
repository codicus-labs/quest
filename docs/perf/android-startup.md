# Android: Kaltstart und Jank (Phase 7)

Gerät: Emulator `quest-api36` (Pixel 7, Android 36 google_apis arm64) auf einem Apple-Silicon-Mac.
Emulator-Zahlen schwanken mehr als auf einem Telefon und sind nur untereinander vergleichbar.

## Methode

- **Kaltstart:** `am force-stop app.quest.android`, dann `am start -W -n app.quest.android/.MainActivity`;
  `TotalTime`, Median über 7–9 Läufe nach einem Aufwärmstart, der nicht zählt. Angemeldet, Daten wie
  im Alltag (Konto mit ~10 Aufgaben heute, Quests, Sitzungen).
- **Macrobenchmark:** `:baselineprofile` (`StartupBenchmark`), `StartupTimingMetric`, COLD, 10 Iterationen,
  `timeToInitialDisplay`. Auf dem Emulator nur mit
  `-Pandroid.testInstrumentationRunnerArguments.androidx.benchmark.suppressErrors=EMULATOR`.
- **Jank:** `dumpsys gfxinfo app.quest.android reset`, dann Plan bzw. Calendar per `adb shell input swipe`
  durchscrollen (Plan: 8 Scrolls nach unten, 12 nach oben; Calendar: 6× nach links, 6× nach rechts wischen, je 4× hoch und
  runter scrollen), danach `dumpsys gfxinfo`. „Janky“ ist Androids eigene Zählung, die Perzentile sind Frame-Zeiten.

**Achtung:** `connectedAndroidTest` des Moduls `:baselineprofile` (Generator und Benchmark) deinstalliert die App
und löscht damit Konto und lokale Daten. Danach neu anmelden, bevor man von Hand misst.

## Kaltstart

| Variante | Median `TotalTime` |
|---|---:|
| Debug (vorher, Phase 6) | 762 ms |
| Release mit R8, ohne Baseline Profile, alter Rust-Build (`release`, aws-lc) | 217 ms |
| Release mit R8, ohne Baseline Profile, `mobile-release` (ring) | 194 ms |
| dieselbe APK, `cmd package compile -m verify` (nichts vorkompiliert) | 203 ms |
| **Release mit Baseline Profile** (`compile -f -m speed-profile` nach profileinstaller) | **168 ms** |

Läufe mit Profil: 169, 168, 169, 152, 168, 167, 161, 168, 173 ms.

Macrobenchmark (leere Daten, `timeToInitialDisplay`):

| Kompilierung | Median | Min | Max |
|---|---:|---:|---:|
| `CompilationMode.None()` | 191,2 ms | 176 ms | 206 ms |
| `Partial(BaselineProfileMode.Require)` | 166,5 ms | 145 ms | 188 ms |

Der große Schritt ist Debug → Release (R8, keine Debug-Checks in Compose, optimierte Rust-Bibliothek); das
Baseline Profile bringt danach ~13 % (194 → 168 ms). Das Profil (`app/src/release/generated/baselineProfiles/`)
deckt Start, Home, Plan, Calendar und die UniFFI-Bindings ab und wird über profileinstaller bei der
Installation bzw. im Hintergrund kompiliert.

## Jank

| Lauf | Frames | Janky | p50 | p90 | p95 | p99 | Slow UI thread |
|---|---:|---:|---:|---:|---:|---:|---:|
| Debug, Plan | 818 | 22 (2,69 %) | 20 ms | 30 ms | 32 ms | 42 ms | 9 |
| Debug, Calendar | 754 | 22 (2,92 %) | 17 ms | 19 ms | 20 ms | 22 ms | 0 |
| Release ohne Profil, Plan | 807 | 26 (3,22 %) | 17 ms | 24 ms | 27 ms | 48 ms | 6 |
| Release ohne Profil, Calendar | 753 | 21 (2,79 %) | 17 ms | 18 ms | 18 ms | 21 ms | 0 |
| **Release mit Profil, Plan** | 845 | 20 (2,37 %) | 17 ms | 18 ms | 18 ms | 21 ms | 0 |
| **Release mit Profil, Calendar** | 755 | 21 (2,78 %) | 17 ms | 18 ms | 19 ms | 21 ms | 0 |

Plan profitiert am meisten: ohne Profil laufen die ersten Scrolls interpretiert/JIT (p99 48 ms, 6 langsame
UI-Frames), mit Profil nicht mehr. Die verbleibenden ~2–3 % „janky“ Frames sind auf dem Emulator auch bei einer
leeren Compose-Liste zu sehen (vsync des Emulators); p50 = 17 ms entspricht 60 Hz.

## Größe

| APK | Größe |
|---|---:|
| Debug (arm64 + x86_64, unoptimierte `.so` nicht gestrippt) | 59,5 MB |
| Release vorher (nur arm64, `release`, aws-lc) | 14,4 MB |
| **Release** (arm64 + x86_64, `mobile-release`, ring, R8) | **19,8 MB** |

`libquest_mobile.so` im Release-APK: arm64-v8a 6,43 MB, x86_64 6,71 MB. Mit nur arm64 (`-Pquest.abis=arm64-v8a`)
wäre das APK ~13 MB.
