# Quest for Android (prototype)

Jetpack Compose UI over the shared Rust core (`crates/quest-mobile` → `quest-store` → `quest-domain`),
bound with UniFFI: Home, Focus, Plan, Quests, Review, Archive, Search, Account and Settings, plus a
home-screen widget, background sync, sharing, shortcuts and a Quick Settings tile. Architecture notes and
per-phase decisions: `docs/mobile.md`; startup and jank numbers: `docs/perf/android-startup.md`.

## Setup (once)

Tools come from `mise.toml` in this folder (Java Temurin 21, Gradle, cargo-ndk); nothing is installed globally.

```sh
cd apps/android
mise trust && mise install
rustup target add aarch64-linux-android x86_64-linux-android
# NDK into the Android SDK (default ~/Library/Android/sdk):
mise exec -- ~/Library/Android/sdk/cmdline-tools/latest/bin/sdkmanager \
  --sdk_root="$HOME/Library/Android/sdk" "ndk;30.0.16248370" "platforms;android-37.2"
echo "sdk.dir=$HOME/Library/Android/sdk" > local.properties
```

Use the SDK's own `sdkmanager` with `--sdk_root`: a Homebrew `sdkmanager` installs into its own
SDK folder, where Gradle does not look.

## Build

```sh
cd apps/android
mise exec -- ./gradlew assembleDebug     # or run ./gradlew in a shell where mise is active
```

Tests (JVM, no emulator): `./gradlew testDebugUnitTest verifyRoborazziDebug`. ViewModel tests use a fake
core; screenshot tests (Roborazzi, light/dark × phone/tablet) compare against `app/src/test/screenshots`.
After an intended UI change, re-record with `./gradlew recordRoborazziDebug`.

Gradle's `cargoBuild` task runs `build-rust.sh`, which

1. builds `libquest_mobile.so` with `cargo ndk` (Cargo profile `mobile-release`) for the ABIs in
   `QUEST_ANDROID_ABIS`, and
2. generates the Kotlin bindings with the crate's `uniffi-bindgen` binary (feature `bindgen`).

Both land in `app/build/rust/` and are wired in as generated sources. `build-rust.sh` also runs
on its own. The ABIs come from the Gradle property `quest.abis` (default `arm64-v8a x86_64`: phones, the
Apple-Silicon emulator and x86_64 emulators/Chromebooks); for faster local builds pass
`-Pquest.abis=arm64-v8a`. Debug and release use the same set, so a debug APK runs where the release does.

## Run

```sh
~/Library/Android/sdk/emulator/emulator -avd quest-api36 &      # AVD: Pixel 7, android-36 google_apis arm64
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n app.quest.android/.MainActivity
```

Create the AVD once with
`avdmanager create avd -n quest-api36 -k "system-images;android-36;google_apis;arm64-v8a" -d pixel_7`.

## Sync against the local server

Without an account the app works as a guest, on this device only. Signing in goes through the browser (a Custom
Tab, RFC 8252): Account → server URL → Sign in opens the server's sign-in page, the identity provider signs you in,
the server asks "Continue as …" for the Android app, and the browser returns to `app.quest.android:/oauth2redirect`
(`AuthRedirectActivity`). Rust (`quest-mobile` over `quest-store`) checks the answer (`state`, `iss`, PKCE), keeps
the tokens through a callback in `auth/TokenFiles.kt` (AES-GCM with a key in the Android Keystore, files in
`noBackupFilesDir`) and switches to the account's own data. After the first sign-in of an account, with guest data
on the device, the app asks whether to add it to the account. Sign out warns when changes have not reached the
server yet, deletes the account's data on this device and ends the browser's sign-in too. Each account keeps its
own data; switching accounts rebuilds the widget, background sync, tile and Focus service on the new data.

Start an identity provider and `quest-server` on the host, each on its own port, and forward both ports to the
emulator. The emulator must reach the server as `127.0.0.1` (not `10.0.2.2`): the server's address is part of the
sign-in answer (`iss`) and its sign-in pages redirect the browser to the provider's address.

```sh
cargo run -p quest-oidc-fake -- --bind 127.0.0.1:8792          # users anna and ben, no passwords
QUEST_BIND=127.0.0.1:8787 QUEST_DATABASE_URL=sqlite:///tmp/quest-dev/quest.sqlite \
QUEST_OIDC_ISSUER=http://127.0.0.1:8792 QUEST_OIDC_CLIENT_ID=quest-server QUEST_OIDC_CLIENT_SECRET=quest-dev-secret \
cargo run -p quest-server
adb reverse tcp:8787 tcp:8787 && adb reverse tcp:8792 tcp:8792
```

The Account screen's server field starts empty in every build: enter `http://127.0.0.1:8787` for the dev server
above, or your own server's https address. Release builds allow plain http only to this device
(loopback); debug builds also to `10.0.2.2`. The rule lives in the core
(`UrlPolicy`, chosen by `BuildConfig.DEBUG` in `RustQuestCore`); `src/debug/res/xml/network_security_config.xml`
mirrors it for Android's own HTTP stack. Release builds have no such file of their own: they use the one of
rustls-platform-verifier, which allows cleartext only to the CAs' CRL servers. The core's HTTPS check fetches CRLs
there for certificates without OCSP (for example Google Trust Services, Let's Encrypt); without it every such server
fails as "invalid peer certificate: Revoked".

A headless desktop replica for checking sync (same account, own data folder; reads commands such as
`add <title>`, `rename <old> => <new>`, `describe`, `move`, `repeat`, `assign`, `start`, `stop`, `delete`,
`restore`, `quit` on stdin, see the head of `examples/desktop_peer.rs`, and prints every change). It signs in as
a user of quest-oidc-fake without a browser:

```sh
cargo run -p quest-mobile --example desktop_peer -- /tmp/peer http://127.0.0.1:8787 anna
```

## Focus and the timer notification

Focus opens from the running time on a task card, the stopwatch icon in the top bar, the timer
notification, or the deep link:

```sh
adb shell am start -a android.intent.action.VIEW -d quest://focus
```

While a timer runs, `FocusService` (foreground service, type `specialUse`) shows an ongoing
notification with Pause/Resume and Complete; it keeps running after the app is swiped from Recents.
The app asks for notification permission once, on the first timer start; without it the service
runs but Android shows no notification. Check the service with
`adb shell dumpsys activity services app.quest.android` (`isForeground=true`, `types=0x40000000`).


## Plan

Compact: one list of days with sticky heads; from 600 dp wide: day columns side by side. Long-press a
card to drag it (to the edge to scroll), or long-press without moving for “Move to…”. Each card also has
accessibility actions to move it. Drag with adb (long press, then move in steps):

```sh
adb shell "input motionevent DOWN 540 1300; sleep 0.8; input motionevent MOVE 540 1500; \
  input motionevent MOVE 540 1700; input motionevent UP 540 1700"
```

Tablet layout on the phone emulator: `adb shell wm size 2560x1600; adb shell wm density 320`, back with
`wm size reset` and `wm density reset`. Numbers: `docs/perf/plan-android.md`.

## Log work

“Log work” (app shortcut, Search) opens a sheet over the current tab and logs the last 15 minutes by
default; the task picker can also create the task.

## Quests, Review, Archive, Search

Quests: as on the desktop, figures (quest hours, in progress, completed), “Active · n” / “Completed · n”
tabs and Archive beside them; cards show colour, name, open tasks and this week's and total hours. Tapping
a card opens the quest editor as a bottom sheet (title, colour, Complete/Reopen quest with
Undo, Open quest page, Delete); closing it keeps the tab and scroll. “New quest” (FAB on phones, header
button on tablets, the dashed card) opens it empty; Create needs only a title. The quest page is for work:
its tasks. Review: arrows or a sideways swipe
turn weeks, a bar under “Recent weeks” jumps to that week. Archive (Quests header, Account, Search)
restores deleted quests, tasks, checklist items and sessions; quests an older version archived are listed with
Open and Unarchive. Search (top bar): results come from the core;
the keyboard's Search runs the first one; “Move to…” switches the list.

A running timer keeps a foreground service alive, so
`am kill` only ends the process once the timer is stopped.

## Widget, background sync, sharing, shortcuts, tile

Widget: long-press the home screen → Widgets → Quest. Narrow or short, it shows today's and this week's
hours; larger, also as many of today's tasks as fit (open before done, Quest Work first). The check toggles a
task, a title opens its sheet (`quest://task/<id>` through the non-exported alias `.InternalEntry`, so other apps
cannot open sheets or start timers), anywhere else opens the app.

Background sync runs through WorkManager: every 15 minutes with network (`quest-sync-periodic`) and once
after leaving the app with unsent changes (`quest-sync-now`). Leaving the app saves local changes first.

```sh
adb shell dumpsys jobscheduler | grep -A3 'app.quest.android/androidx.work'   # job ids
adb shell cmd jobscheduler run -f -n androidx.work.systemjobscheduler app.quest.android <jobId>
adb shell "am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT 'Read this' app.quest.android"
adb shell am start -a app.quest.android.action.LOG_WORK -n app.quest.android/.MainActivity
adb shell cmd statusbar add-tile app.quest.android/.platform.NewTaskTileService
```

WorkManager postpones a forced periodic job that is not due yet; the woken process still syncs live.
Use `run-as app.quest.android kill <pid>` to end the process without `force-stop` (which cancels jobs
and alarms). Shortcuts (long-press the app icon): New task, Log work, Start timer (last task). The Quick
Settings tile “New task” opens the same quick-add sheet as sharing.

Testing tips: test tags are exposed as resource ids (also inside the task sheet), so
`adb shell uiautomator dump` finds them. Start swipes away from the screen edge, or the system back
gesture takes them. To test process death, put the app in the background, `adb shell am kill
app.quest.android`, and return through Recents (`am start` opens a new task without saved state).

## AI capture

Dictate or paste text, let the server's AI turn it into quests, tasks and time entries, check and edit the
proposal, then Create. Entry points: the mic in quick add's title, the share target "Structure with AI", the
"Dictate" shortcut (long-press the app icon) and "Capture with AI" in the search. They show only when the
signed-in account turned on Settings → AI features (off by default, kept by the server for all devices) and the
server has an AI provider. Turning AI features on shows the privacy note first; the mic permission is asked on the
first tap on Dictate, after our own explanation.

The server needs `QUEST_AI_*` (see `crates/quest-server/README.md`). To try it without a provider, run the
mock and a second server on their own ports (with `QUEST_OIDC_*` as above), forward the port with `adb reverse`,
then sign in on the emulator with `http://127.0.0.1:8797`:

```sh
QUEST_AI_MOCK_BIND=127.0.0.1:8798 cargo run -p quest-server --example ai_mock
QUEST_BIND=127.0.0.1:8797 QUEST_DATABASE_URL=sqlite:///tmp/p8a-server/quest.sqlite \
QUEST_AI_BASE_URL=http://127.0.0.1:8798/v1 QUEST_AI_API_KEY=mock QUEST_AI_MODEL=mock \
QUEST_OIDC_ISSUER=http://127.0.0.1:8792 QUEST_OIDC_CLIENT_ID=quest-server QUEST_OIDC_CLIENT_SECRET=quest-dev-secret \
cargo run -p quest-server
adb shell "am start -a android.intent.action.SEND -t text/plain \
  --es android.intent.extra.TEXT 'Write the offer tomorrow' -n app.quest.android/.capture.StructureWithAi"
```

After Create a snackbar offers Undo for about six seconds; Undo deletes what that capture created (what still
exists). Without an AI provider on the server the entry points stay hidden.
The emulator has no usable microphone, so dictation needs a real device.

## Connected AI clients

Settings → Connected AI clients (signed in only) lists the AI apps connected to the account over MCP
(`docs/mcp.md`), with their access (read only, or read and write) and when they were connected and last used. Revoke asks first, then cuts the client off at once.
The list is read from the server on every visit; offline it says so and offers Try again.

## Release build

The version lives in `version.properties` (`versionName`); `versionCode` is derived from it
(`major * 1 000 000 + minor * 1 000 + patch`). Release builds run R8 (`app/proguard-rules.pro` keeps what JNA,
UniFFI, rustls-platform-verifier and Glance reach by name) and resource shrinking.

Signing comes from `~/.gradle/gradle.properties` or environment variables, never from the repository (the build
stops if one of the names is in `apps/android/gradle.properties`; the Rust build does not see them):

| Name | Value |
|---|---|
| `QUEST_RELEASE_STORE_FILE` | path to the keystore (`.jks`) |
| `QUEST_RELEASE_STORE_PASSWORD` | keystore password |
| `QUEST_RELEASE_KEY_ALIAS` | key alias |
| `QUEST_RELEASE_KEY_PASSWORD` | key password |

Without `QUEST_RELEASE_STORE_FILE` the release APK is built unsigned; with it, all four are required.

```sh
# A throwaway key for trying the release build (never for publishing):
mise exec -- keytool -genkeypair -keystore /tmp/quest-release-test.jks -alias quest-test \
  -keyalg RSA -keysize 4096 -validity 365 -storepass questtest -keypass questtest -dname "CN=Quest test"
QUEST_RELEASE_STORE_FILE=/tmp/quest-release-test.jks QUEST_RELEASE_STORE_PASSWORD=questtest \
QUEST_RELEASE_KEY_ALIAS=quest-test QUEST_RELEASE_KEY_PASSWORD=questtest \
  mise exec -- ./gradlew assembleRelease
adb install -r app/build/outputs/apk/release/app-release.apk
```

An APK signed with another key cannot update an installed one: uninstall first (this deletes local data).

Dependencies are checked against `gradle/verification-metadata.xml` (SHA-256). After adding or bumping a
dependency, regenerate it with the tasks that resolve it, e.g.
`./gradlew --write-verification-metadata sha256 assembleDebug assembleRelease testDebugUnitTest verifyRoborazziDebug`,
and review the diff.

## Baseline Profile and startup benchmark

`:baselineprofile` (a `com.android.test` module) drives the app through start, Home and Plan.
The generated profile is checked in (`app/src/release/generated/baselineProfiles/`) and installed by
profileinstaller. Regenerate after larger UI changes, with the emulator running:

```sh
mise exec -- ./gradlew :app:generateReleaseBaselineProfile
mise exec -- ./gradlew :baselineprofile:connectedBenchmarkReleaseAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=app.quest.android.baselineprofile.StartupBenchmark \
  -Pandroid.testInstrumentationRunnerArguments.androidx.benchmark.suppressErrors=EMULATOR
```

Both uninstall the app at the end, which deletes the account and local data. Methods and numbers:
`docs/perf/android-startup.md`.

## Releases on GitHub

`.github/workflows/android-release.yml` runs on a tag `android-v<versionName>` (it checks the tag against
`version.properties`) and builds the release APK unsigned, so Gradle plugins and crates never see a secret.
A separate step signs it with `sign-release.sh` (`zipalign -P 16`, `apksigner`): the keystore from the
repository secrets (`QUEST_RELEASE_KEYSTORE_BASE64`, `QUEST_RELEASE_STORE_PASSWORD`, `QUEST_RELEASE_KEY_ALIAS`,
`QUEST_RELEASE_KEY_PASSWORD`) exists only during that step. The script then compares the signing certificate's
SHA-256 with the repository variable `QUEST_RELEASE_CERT_SHA256` and stops on a mismatch or a missing value.
The job attaches `quest-<version>.apk` and its `.sha256` to a GitHub release. Only the small release job may write to the repository; the build job reads only and keeps no
Git credentials. A re-run replaces the files of an existing release. To release: bump `versionName`, commit, `git tag android-v0.1.1 && git push origin android-v0.1.1`.

Both jobs run on GitHub-hosted runners (`ubuntu-latest`). The build job installs the tools of `mise.toml` with
`jdx/mise-action`, the toolchain of `rust-toolchain.toml` with the Android targets with `rustup`, and the NDK,
platform and build-tools with the runner's `sdkmanager`; Cargo, Gradle and mise are cached between runs. After
changing the NDK, platform or build-tools in `app/build.gradle.kts`, update the `sdkmanager` step of the workflow
too. Without the four signing secrets the build job stops at its first step with an error naming them.

Dry run without a tag: `gh workflow run android-release.yml --ref main`, then `gh run watch`. It builds, signs and
checks the APK of `version.properties` like a release (the log ends the signing step with "Signed with the
expected certificate …") and keeps `quest-<version>.apk` with its `.sha256` as the artifact `apk` for 7 days
(`gh run download <run-id> -n apk`), but creates no release.

### Who can release

The workflow has no `environment:`. The protection rests on these:

- The signing secrets and the variable `QUEST_RELEASE_CERT_SHA256` are repository-wide: every workflow run on a
  branch or tag of the repository could read them, so only people with write access can push a workflow or a tag
  that uses them. Pull requests from forks get no secrets, and no workflow uses `pull_request_target`.
- The release workflow publishes only on a tag `android-v*` (and checks it against `version.properties`); a manual
  run is a dry run that signs with the same secrets but creates no release, and there is no branch or pull request
  trigger.
- The fingerprint check: `sign-release.sh` stops unless the signed APK's certificate SHA-256 equals
  `QUEST_RELEASE_CERT_SHA256`, so a swapped keystore secret cannot publish an APK under another key.

With more than one person holding write access, move the secrets into an Environment with required reviewers and
add `environment:` to the build job.

### Sign-in in release builds

The APK contains no server address: the Account screen's server field starts empty in every build, and every build
signs in through the private-use scheme `app.quest.android:/oauth2redirect` (RFC 8252 §7.1), whichever server is
entered. The server always asks "Continue as …" for the Android client, and PKCE binds the answer to the app that
started the sign-in. Signing out returns the same way.

App Links (RFC 8252 §7.2) stay an optional server feature: with `QUEST_ANDROID_CERT_SHA256` set, `quest-server`
serves `/.well-known/assetlinks.json` and accepts `{origin}/auth/android` as a redirect (`docs/auth.md`,
`docs/deployment.md`). This app does not use it. A build that wants it needs an intent filter with
`android:autoVerify="true"` for that address on `AuthRedirectActivity` and must pass the server's origin to the core
(`app_link_origin` of `begin_sign_in`, `AppClient::android` in `quest-store`).

## Install and updates (Obtainium)

Quest is not in an app store. [Obtainium](https://github.com/ImranR98/Obtainium) installs and updates it from
the GitHub releases:

1. Install Obtainium on the phone (its own GitHub releases or F-Droid).
2. *Add app* → the repository's GitHub URL. Leave the APK filter empty (each release has one APK) or set
   `quest-.*\.apk`.
3. Obtainium installs the latest release and checks for new tags; *Update* installs the new APK over the old
   one. Updates only work while every release is signed with the same key.
4. Optional: compare the certificate with the release key's SHA-256 (the value of `QUEST_RELEASE_CERT_SHA256`,
   also printed in the release job's log): `apksigner verify --print-certs quest-<version>.apk`, or Obtainium's
   app info.

Without Obtainium: download the APK from the release page and open it (allow installing from that source), or
`adb install -r quest-<version>.apk`.

## Accessibility

Screen titles and section headings are `heading()`s; cards have accessibility actions for what touch does by
swipe or drag (complete, delete, move). Text scales
with the system font size (checked at 200 % in Roborazzi: `*-font200-light.png`); only the bottom navigation
labels (130 %) are capped. Password fields are marked for autofill.

## License

MIT, like the rest of the repository: see `LICENSE` in the repository root. The bundled Inter font keeps its own
license (`INTER-LICENSE.txt`).
