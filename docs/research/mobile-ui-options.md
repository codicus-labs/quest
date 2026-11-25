# Mobile-UI-Optionen für Quest (Android, später iOS)

Stand: 2026-11-23. Fakten aus Primärquellen (Repos, READMEs, Source, Release-Daten via GitHub API);
jede Aussage verweist auf eine Quelle in eckigen Klammern, siehe [Quellen](#quellen). Aufwandsschätzungen
sind eigene Einschätzungen und als solche markiert.

## Kurzfazit

**Empfehlung 1 – Shared Rust Core + native UI-Shells (Discords tatsächlicher Weg):**
`quest-domain` + `quest-store` bleiben unverändert. Dazu kommt ein dünnes FFI-Crate (`quest-mobile`,
UniFFI) über der bestehenden `Store`-Fassade (`open/snapshot/dispatch/status/…`, `on_change` →
Callback-Interface). Android-UI in Jetpack Compose, iOS später in SwiftUI – oder beides in einer
Codebasis via Compose Multiplatform + Gobley. Rune bleibt für macOS.
Gründe: Nur dieser Weg liefert *heute* produktionsreife Touch-, IME-, Drag-and-Drop- und
Accessibility-Unterstützung auf Android. UniFFI läuft in Firefox mobile in Produktion [U1].
`quest-store` nutzt bereits `rustls` + `webpki-roots` statt `native-tls` und braucht deshalb kein
Android-spezifisches TLS-Setup [Q1]. Eine Phone-UI (kein ⌘K, keine Hover, schmaler Screen) muss
ohnehin neu entworfen werden, die Rune-Views wären also auch mit Mobile-Backend nur begrenzt
wiederverwendbar.

**Empfehlung 2 – nur falls „alles in Rust, eine UI-Codebasis“ Pflicht ist: Slint.**
Slint dokumentiert Android (API 26+) und iOS als Plattformen und blendet die Soft-Tastatur selbst
ein und aus [S1][S2][S3]. Es hat eine Royalty-free-Lizenz für Mobile, die Attribution verlangt [S4].
Das bedeutet aber einen kompletten UI-Rewrite inklusive Desktop, sonst pflegt man zwei UIs.

**Rune selbst ist für Android derzeit keine Option.** Die README nennt nur macOS, Windows, Linux und Web
[R1]. Android ist der letzte Meilenstein M7 im PLAN, ohne Datum [R2]. iOS (M6) existiert erst als
Embed-Layer (`LayerHost` auf `CAMetalLayer`) [R3] und als offener Fork-Vorschlag ohne Soft-Tastatur,
Safe-Area und VoiceOver [R4].

**Discord/GPUI-Behauptung: falsch.** Discord nutzt GPUI nur für den Desktop-Client. Mobile behält
native UI und teilt nur den App-Code über Rust. Einen „GPUI-Adapter, der native UI bindet“ hat
Discord nicht beschrieben (siehe [Faktencheck](#faktencheck-discord--gpui)).

## Vergleichstabelle

| Option | Android | iOS | Letzte Release | Lizenz | `quest-domain`/`quest-store` unverändert? | Migrationsaufwand (Schätzung) |
|---|---|---|---|---|---|---|
| **Rust Core + UniFFI + Compose/SwiftUI** | Produktion (Firefox via UniFFI [U1]) | Produktion | UniFFI v0.32.2 [U2] | MPL-2.0 (UniFFI) | Ja, plus FFI-Crate | Mittel: neue Mobile-UI pro Plattform, Desktop bleibt Rune |
| **… + Compose Multiplatform + Gobley** | Jetpack Compose | Stable seit CMP 1.8.0 (2025-05) [C1] | CMP 1.12.2 (2026-10-15) [C2]; Gobley 0.3.7 (2025-10) [G1] | Apache-2.0 / MPL-2.0 | Ja | Mittel: eine Kotlin-UI für beide Plattformen; Gobley ist klein und pre-1.0 |
| **Crux** | über native Shell | über native Shell | crux_core 0.20.0 (2026-08-07) [X2] | Apache-2.0 | Domain ja; Store **nein** (IO soll in der Shell laufen) [X1] | Hoch: Store-Umbau auf das Effects-Modell |
| **Rune + eigenes Android-Backend** | kein Backend, M7 [R2] | Embed-Layer + Fork-PR [R3][R4] | keine (git-Rev, „pre-release“) [R1] | MIT | Ja | Sehr hoch: Shell, IME, Touch, A11y, Packaging selbst bauen |
| **Rune Web-Target im Android-Browser** | experimentell [R5][R6] | – | – | MIT | **Nein**: rusqlite/tokio-Threads laufen nicht in wasm32 | Hoch: Store-Backend für den Browser nötig |
| **Slint** | dokumentiert, API 26+ [S1] | dokumentiert [S2] | v1.19.0 (2026-11-16) [S5] | GPLv3 / Royalty-free mit Attribution / kommerziell [S4] | Ja | Hoch: kompletter UI-Rewrite in `.slint` |
| **Dioxus (mobil = WebView)** | `dx serve --platform android` [D1] | ja [D1] | v0.7.10 (2026-07-30); 0.8.0-alpha.3 [D2] | MIT/Apache-2.0 | Ja | Hoch: UI-Rewrite in RSX/HTML/CSS; Desktop könnte mitwandern |
| **Dioxus Native / Blitz** | early/beta [D3] | keine Angabe | v0.3.0-beta.4 [D3] | Apache-2.0 | Ja | Hoch + Reifegradrisiko |
| **Tauri 2 mobile** | Android 8+ (System WebView) [T1] | iOS (WKWebView) [T1] | v2.12.7 (2026-11-18) [T2] | MIT/Apache-2.0 | Ja | Hoch: Web-Frontend neu (JS/TS) |
| **GPUI (Zed)** | **nicht unterstützt**; Android-PR geschlossen [P2] | `gpui_ios` seit 2026-10-14 [P1] | keine eigenen Releases; pre-1.0 [P3] | Apache-2.0 | Ja | Sehr hoch und für Android nutzlos |
| **NSH (Native Shell, Longbridge)** | Android Views, API 26+ [N1] | UIKit [N1] | v0.6.1 (2026-11-20) [N2] | Apache-2.0 [N1] | Ja | Hoch + Reifegradrisiko (Repo seit 2026-10) |
| **Expo „Exact“** | „Works“, Material-3-Feinschliff läuft [E1] | UIKit [E1] | keine Release, pre-1.0 [E1] | MIT | Ja (Rust-Datenlogik möglich) [E1] | Hoch: UI in eigener Sprache „Contract“ |
| **Makepad** | `cargo makepad android` [M1] | `cargo makepad apple ios` [M1] | Tag release-2026-11-06.5 [M2] | MIT | Ja | Hoch |
| **egui/eframe** | ja, Soft-Keyboard-Issue offen [EG1][EG2] | kein offizielles Target [EG3] | 0.37.0 (2026-10-15) [EG4] | MIT/Apache-2.0 | Ja | Hoch, Immediate-Mode-Look |
| **Iced** | kein offizieller Support [I1] | nein | 0.14.0 (2025-12-07) [I2] | MIT | Ja | – |
| **Freya** | „highly experimental“, keine Soft-Tastatur/IME [F1] | nein | v0.5.4 (2026-11-22) [F2] | MIT | Ja | – |
| **Xilem/Masonry** | Beispiele via NativeActivity; Tastatur nicht aus Rust auslösbar [XI1] | nein | v0.4.0 (2025-10-29) [XI2] | Apache-2.0 | Ja | – |

## Ausgangslage im Repo

- `quest-store` ist UI-agnostisch und hat eine schmale Fassade (`Store::open(config, on_change)`,
  `snapshot()`, `dispatch(Command)`, `status()`, `sign_in/out`) [A1]. Diese Form passt direkt zu
  UniFFI: Objekt + Methoden + Callback-Interface für `on_change` [U3].
- Abhängigkeiten: `rusqlite` (`bundled`), `tokio` (`rt`, `net`, `time`), `tokio-tungstenite`
  (`rustls-tls-webpki-roots`) und `reqwest` (`rustls`) [Q1].
  - `bundled` kompiliert SQLite über das `cc`-Crate aus dem Quellcode [RS1]. Für Android braucht das
    die NDK-Toolchain, z. B. via `cargo-ndk` [ND1].
  - Mit `webpki-roots` sind die CA-Roots einkompiliert. Die Android-Kotlin-Komponente, die
    `rustls-platform-verifier` braucht [RV1], entfällt deshalb.
- Wiederverwendbar über die Crates hinaus: die puren State-Machines in `quest-app` (`dnd::step`,
  Undo, Palette-Ranking). Für Compose/SwiftUI müssten sie in ein geteiltes Crate wandern oder neu
  geschrieben werden. Die Rune-Views (~12k Zeilen in `crates/quest-app/src`, inkl. Tests) sind
  nicht portierbar.
- Rune-spezifische Workarounds aus `crates/quest-app/RUNE_NOTES.md` (eigenes Drag and Drop über
  Drop-Zone-Rects, eigene Toasts, eigene Palette) zeigen: Auch auf Desktop baut die App viel selbst,
  was native Mobile-Toolkits mitbringen.

## Optionen im Detail

### 1. Rune

- **Plattformen heute.** Die README beschreibt Rune als „Rust UI framework for macOS, Windows, Linux
  and the web“ [R1].
- **Reihenfolge laut PLAN.** Desktop → Web → iOS → Android. Die Meilensteine sind „M6 — iOS“
  (iOS-Shell, `UITextInput`, `RuneView` zum Einbetten in SwiftUI) und als letzter „M7 — Android“,
  ohne Inhalt oder Datum [R2].
- **iOS-Stand im Repo.** `rune::embed::LayerHost` kann auf einen bestehenden `CAMetalLayer` rendern.
  `rune` baut für `aarch64-apple-ios`, aber das App-Fenster, die Display-Link-Taktung und die
  Touch-Weiterleitung muss der Host selbst schreiben [R3].
- **Issue #803 (offen).** `App::run` hat keinen iOS-Zweig, und `platform/` keinen iOS-Ordner [R4].
  Ein Fork-Branch liefert `RuneView` + `CADisplayLink`, aber ohne Soft-Tastatur, Safe-Area,
  Multitouch, VoiceOver oder Clipboard [R4].
- **Android im Source.** Es gibt nur `cfg`-Ausschlüsse (z. B. kein `arboard`, Fonts), keine
  Plattform-Shell [R3][R7].
- **Winit.** Rune vendort einen gepatchten winit [R8]. Upstream-winit listet Android und iOS als
  Plattformen [W1]. Runes eigene Recherche verwirft winit aber für Mobile, weil `winit-uikit` kein
  `UITextInput` implementiert (also keine Autokorrektur, kein CJK-IME, kein Diktat und keine
  Selection-Handles), und plant eigene dünne Shells [R9].
- **Accessibility.** Es gibt ein offenes Issue zu WCAG für iOS und Android [R10]. Laut PLAN sind die
  iOS- und Android-Adapter von AccessKit noch jung [R2].
- **Web-Target.** Rune rendert im Browser über WebGPU bzw. WebGL2 in einen Canvas, mit Touch-Pan und
  Mobile-`beforeinput` [R5]. Ein offenes Issue betrifft Performance unter Android-Chrome [R6]. Das
  wäre ein PWA-Weg ohne Store-Port; `quest-store` (Threads, rusqlite, Tokio-Netzwerk) läuft aber
  nicht unter `wasm32-unknown-unknown`. Man bräuchte einen zweiten Store auf IndexedDB/OPFS-Basis.
- **Repo-Aktivität.** Es gibt sehr viele Commits pro Tag (mehrere am 2026-11-23) [R11]; die Lizenz
  ist MIT.
- **Aufwand (Schätzung).** Ein eigenes Android-Backend hieße: Activity/Surface-Lifecycle,
  `InputConnection`-IME, Multitouch und Gesten, Safe-Area- und Keyboard-Insets, AccessKit-Android,
  Gradle/APK-Packaging. Das sind Framework-Arbeiten im Umfang von Monaten, gegen ein Upstream, das
  sich täglich bewegt. Nicht empfohlen. Realistisch ist nur abwarten oder upstream beitragen.

### 2. GPUI (Zed)

- **iOS.** Ein offizielles Backend `crates/gpui_ios` wurde am 2026-10-14 gemerged (PR #63068) [P1].
  Danach folgten Gesten, Fling und Scroll in einzelnen PRs [P1].
- **Android.** Nicht unterstützt. PR #61057 „Add Android platform support“ wurde am 2026-08-08
  ungemerged geschlossen [P2]. Es gibt kein `gpui_android`-Crate.
- **Reife.** Das GPUI-README sagt pre-1.0 mit häufigen Breaking Changes [P3]. Die Lizenz ist
  Apache-2.0 (laut Crate-Manifest) [P3].
- **gpui-component.** Das Projekt heißt jetzt `gpui-kit`, v0.11.0 erschien am 2026-11-23. Die README
  nennt nur macOS, Windows, Linux und Web [P4].
- **Fazit.** Für ein Android-Ziel ungeeignet. Der Wechsel von Rune zu GPUI brächte keinen
  Mobile-Vorteil.

### 3. Dioxus / Dioxus Native (Blitz)

- **Mobile.** Die README verspricht „First-class Android and iOS support“. `dx serve --platform
  android` und `dx bundle` erzeugen `.apk`/`.ipa`. Gerendert wird standardmäßig in der System-WebView
  (wry) [D1].
- **Releases.** Stabil ist v0.7.10 (2026-07-30), dazu gibt es v0.8.0-alpha.3 (2026-10-16) [D2]. Die
  Lizenz ist MIT/Apache-2.0 [D1].
- **Blitz (Dioxus Native).** Status „beta“; die README spricht von „many bugs and missing features“.
  Android-Accessibility und Touch-Fling liegen als offene PRs vor [D3].
- **Passung.** `quest-store` läuft in-process als normale Rust-Lib, weil die Rust-Seite nativ
  kompiliert wird. Die UI muss komplett in RSX + CSS neu geschrieben werden; Touch, IME und A11y
  kommen dann von der WebView.

### 4. Slint

- **Android.** Dokumentiert über `backend-android-activity-06`, mindestens API 26 [S1].
- **iOS.** Dokumentiert über das winit-Backend + Skia; das braucht macOS und Xcode [S2].
- **Soft-Tastatur.** Wird bei Fokus automatisch geöffnet. `virtual-keyboard-position` und
  `virtual-keyboard-size` sind als Properties verfügbar [S3].
- **Lizenz.** Es gibt drei Varianten [S4]:
  - GPLv3.
  - Royalty-free: proprietäre Desktop-, Mobile- und Web-Apps sind erlaubt, Embedded nicht. Pflicht
    ist das `AboutSlint`-Widget im About- oder Splash-Screen plus ein Attribution-Badge auf einer
    öffentlichen Webseite.
  - Kommerziell.
- **Releases.** v1.19.0 am 2026-11-16, sehr aktiv [S5].
- **Passung.** `quest-domain` und `quest-store` laufen unverändert. Die gesamte UI muss in der
  Slint-DSL neu entstehen. Drag and Drop und Accessibility auf Mobile habe ich nicht verifiziert.

### 5. Iced, egui/eframe, Makepad, Freya, Xilem/Masonry

- **Iced.** Kein offizieller Mobile-Support; Issue #302 „Mobile support“ ist offen [I1]. Release
  0.14.0 am 2025-12-07 [I2].
- **egui/eframe.**
  - Android läuft über `android-game-activity` bzw. `android-native-activity` [EG1].
  - Offene Issues: „Keyboard not opening under Android“ [EG2] und „Add iOS native target“ [EG3].
  - AccessKit nur unter Windows und macOS [EG5].
  - Release 0.37.0 am 2026-10-15 [EG4].
- **Makepad.**
  - Android- und iOS-Toolchains sind in der README dokumentiert [M1].
  - Release-Tag release-2026-11-06.5 [M2].
  - Produktionsnahes Beispiel ist der Matrix-Client Robrix [M3].
  - Mobile-IME und A11y nicht verifiziert.
- **Freya.** Android-Beispiel mit dem Hinweis „highly experimental. Soft keyboard and IME support
  are not yet implemented“ [F1].
- **Xilem/Masonry.**
  - Android-Beispiele laufen über NativeActivity; laut Maintainern lässt sich die virtuelle Tastatur
    dort nicht aus Rust auslösen [XI1].
  - Ein Beispiel stürzt auf Android leicht ab [XI3].
  - Release v0.4.0 am 2025-10-29 [XI2].
- **Fazit.** Keines davon ist für eine textlastige Planer-App auf Android reifer als Slint.

### 6. Tauri 2 mobile

- **Plattformen.** Tauri nutzt wry und tao, mit WKWebView auf iOS und Android System WebView ab
  Android 8 [T1].
- **Releases.** v2.12.7 am 2026-11-18, sehr aktiv [T2].
- **Plugins.** Rust-Plugins mit Kotlin- und Swift-Bindings; pro Plattform wird ein Support-Level
  deklariert [T3].
- **Passung.** Die Rust-Lib läuft in-process, Tokio nutzt Tauri selbst. Das Frontend wäre eine neue
  Web-App (JS/TS), mit Touch, IME und A11y aus der WebView. Sinnvoll nur, wenn man eine Web-UI ohnehin
  will.

### 7. Shared Rust Core + native UI (UniFFI, Crux, Compose Multiplatform)

- **UniFFI.**
  - Generiert Kotlin- und Swift-Bindings. Async ist unterstützt: Rust `async fn` → Kotlin `suspend` /
    Swift `async` [U4].
  - Callback-Interfaces erlauben fremd implementierte Traits [U3].
  - Die README sagt „ready for production use“, aber noch weit von 1.0; Mozilla nutzt es in Firefox
    mobile und desktop [U1].
  - Lizenz MPL-2.0. Neuester Tag v0.32.2 [U2].
- **Passung zu Quest.**
  - Ein neues Crate `quest-mobile` exportiert ein `QuestStore`-Objekt sowie einen `on_change`-Callback
    (Kotlin ruft dann `snapshot()` erneut ab, analog zu `AppHandle::wake()`).
  - Den Snapshot liefert es als flache Records bzw. ViewModels.
  - `Command` wird ein UniFFI-Enum.
  - `quest-domain` und `quest-store` bleiben unverändert.
  - Policy, die heute in `quest-app` liegt (z. B. `dnd::step`), kann bei Bedarf in ein geteiltes,
    pures Crate wandern.
- **Compose Multiplatform + Gobley.**
  - iOS ist seit CMP 1.8.0 (2025-05-06) „stable and production-ready“ [C1]; aktuell v1.12.2 [C2].
  - Gobley liefert UniFFI-Bindings für Kotlin Multiplatform plus ein Gradle-Plugin, das Rust baut und
    linkt (v0.3.7, 2025-10-08) [G1].
  - Ergebnis: eine Kotlin-UI für Android und iOS über demselben Rust-Core.
  - Risiko: Gobley ist klein und seit über einem Jahr ohne Release.
- **Crux.**
  - Elm-artiger Core (`update(event, &mut model) -> Command<Effect, Event>`) mit Shells in SwiftUI,
    Compose und Web [X1].
  - IO läuft per Design in der Shell; Effects sind Werte, die die Shell ausführt [X1].
  - Laut README „production-ready, but occasional breaking changes“; v0.20.0 erschien am 2026-08-07
    [X2]. Die aktuelle README nutzt BoltFFI statt UniFFI für die Bindings [X1].
  - Passung: `quest-store` macht IO selbst (Tokio, SQLite, WebSocket). Crux würde einen Umbau des
    Stores auf das Effects-Modell verlangen, deshalb ist UniFFI direkt hier passender.
- **Touch, IME, Drag and Drop, A11y.** Kommen vollständig von Compose bzw. SwiftUI. Compose hat
  eingebaute Drag-and-Drop- und Accessibility-APIs; das ist Plattformstandard und nicht separat
  zitiert.

### 8. Native-Widget-Adapter in Rust

- **NSH (Native Shell) von Longbridge**, den Autoren von gpui-component.
  - Ein Rust-UI-Framework, das auf jeder Plattform native Views rendert: UIKit auf iOS, Android Views
    auf Android, AppKit, Win32, GTK 4. Zusätzlich gibt es ein **GPUI-Backend** (`nsh-platform-gpui`),
    mit dem sich NSH in GPUI-Apps einbetten lässt [N1].
  - Apps entstehen in Rust oder TSX [N1]. Lizenz Apache-2.0, Repo seit 2026-10-14, v0.6.1 am
    2026-11-20 [N1][N2].
  - Kommt dem Bild „GPUI + Adapter zu nativer UI“ am nächsten, hat aber nichts mit Discord zu tun und
    ist sehr jung.
- **Expo „Exact“.**
  - UIKit und AppKit nativ; auf Android Canvas (`RenderNode`) mit Material-3-Styling plus native
    `EditText`, Dialoge und Compose-Datepicker [E1].
  - Die UI wird in der eigenen Sprache „Contract“ deklariert; Datenlogik in TypeScript oder Rust [E1].
  - „pre-1.0 and changes daily“, keine Releases [E1].
- **Fazit.** Interessant zum Beobachten, aber zu jung für eine Produktentscheidung.

## Faktencheck Discord / GPUI

**Behauptung:** „Discord nutzt GPUI mit einem Adapter, um auf Mobile native UI zu binden.“

**Ergebnis: falsch bzw. ungenau.**

- **Primärquelle** ist ein X-Post von Discord-CTO Stanislav Vishnevskiy vom 2026-11-04 [DC1]:
  - „That means Electron goes away, and so does React Native, mobile keeps native UI but shares app
    code with desktop via Rust.“
  - „(And it's not Tauri, it's GPUI …)“
- **GPUI betrifft nur den Desktop-Client** (Codename „Phoenix“). Mobile nutzt native iOS- und
  Android-Views; geteilt wird der App-Code (Rust-Core), nicht die UI [DC2][DC3].
- **Ein Adapter, der GPUI-UI auf native Mobile-Widgets abbildet, wird nirgends beschrieben** [DC2][DC3].
  Discord hat bisher keinen eigenen Engineering-Blogpost dazu veröffentlicht. GPUI selbst hat kein
  Android-Backend [P2].
- **Was stimmt:** Discords Architektur ist genau „Shared Rust Core + native Mobile-UI“, also
  Empfehlung 1 oben, und GPUI auf dem Desktop.
- **Einschränkung zur Quelle.** Den X-Post konnte ich nicht direkt abrufen (X liefert ohne Login
  HTTP 402). Das Zitat ist über das eingebettete Tweet-Zitat bei Dexerto sowie InfoQ und heise
  abgeglichen [DC2][DC3][DC4].

## Unsicherheiten

- Wie die Quest-Domäne (Fractional Index, HLC, Snapshot-Typen) über UniFFI abgebildet wird, ist nicht
  prototypisiert. Das wäre ein Spike von 1–2 Tagen: `quest-mobile`-Crate, Android-App mit Liste +
  `dispatch`.
- Drag and Drop, IME und A11y auf Mobile sind für Slint, Makepad, NSH und Exact nicht praktisch
  geprüft.
- Wie schnell Rune Android liefert, ist offen. Der PLAN nennt keine Termine [R2].

## Quellen

Repo:
- [A1] `docs/ARCHITECTURE.md` (Vertrag `quest-store`)
- [Q1] `crates/quest-store/Cargo.toml`

Rune:
- [R1] https://github.com/anomalyco/rune/blob/dev/README.md
- [R2] https://github.com/anomalyco/rune/blob/dev/PLAN.md (Plattform-Reihenfolge, M6/M7, Risiken)
- [R3] https://github.com/anomalyco/rune/blob/dev/changelog/unreleased/2026-10-30-ios-embed.md
- [R4] https://github.com/anomalyco/rune/issues/803
- [R5] https://github.com/anomalyco/rune/blob/dev/WEB.md
- [R6] https://github.com/anomalyco/rune/issues/1670
- [R7] https://github.com/anomalyco/rune/blob/dev/crates/rune/Cargo.toml
- [R8] https://github.com/anomalyco/rune/tree/dev/vendor
- [R9] https://github.com/anomalyco/rune/blob/dev/research/11-platform-shells.md
- [R10] https://github.com/anomalyco/rune/issues/110
- [R11] https://github.com/anomalyco/rune/commits/dev

GPUI:
- [P1] https://github.com/zed-industries/zed/pull/63068 , https://github.com/zed-industries/zed/tree/main/crates/gpui_ios
- [P2] https://github.com/zed-industries/zed/pull/61057 , https://github.com/zed-industries/zed/issues/43207
- [P3] https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md , https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml
- [P4] https://github.com/longbridge/gpui-component

Dioxus:
- [D1] https://github.com/DioxusLabs/dioxus/blob/main/README.md
- [D2] https://github.com/DioxusLabs/dioxus/releases
- [D3] https://github.com/DioxusLabs/blitz

Slint:
- [S1] https://github.com/slint-ui/slint/blob/master/docs/astro/src/content/docs/guide/platforms/mobile/android.mdx
- [S2] https://github.com/slint-ui/slint/blob/master/docs/astro/src/content/docs/guide/platforms/mobile/ios.mdx
- [S3] https://github.com/slint-ui/slint/blob/master/docs/astro/src/content/docs/guide/platforms/mobile/general.mdx
- [S4] https://github.com/slint-ui/slint/blob/master/LICENSE.md , https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md
- [S5] https://github.com/slint-ui/slint/releases

Iced:
- [I1] https://github.com/iced-rs/iced/issues/302
- [I2] https://github.com/iced-rs/iced/releases

egui:
- [EG1] https://github.com/emilk/egui/blob/main/crates/eframe/Cargo.toml
- [EG2] https://github.com/emilk/egui/issues/5590
- [EG3] https://github.com/emilk/egui/issues/3117
- [EG4] https://github.com/emilk/egui/releases
- [EG5] https://github.com/emilk/egui/blob/main/README.md

Makepad:
- [M1] https://github.com/makepad/makepad/blob/main/README.md
- [M2] https://github.com/makepad/makepad/tags
- [M3] https://github.com/project-robius/robrix

Freya:
- [F1] https://github.com/marc2332/freya/blob/main/examples/android/README.md
- [F2] https://github.com/marc2332/freya/releases

Xilem:
- [XI1] https://github.com/linebender/xilem/issues/1780
- [XI2] https://github.com/linebender/xilem/releases
- [XI3] https://github.com/linebender/xilem/issues/1377

winit:
- [W1] https://github.com/rust-windowing/winit/blob/master/FEATURES.md

Tauri:
- [T1] https://github.com/tauri-apps/tauri/blob/dev/README.md
- [T2] https://github.com/tauri-apps/tauri/releases
- [T3] https://v2.tauri.app/develop/plugins/

UniFFI:
- [U1] https://github.com/mozilla/uniffi-rs/blob/main/README.md
- [U2] https://github.com/mozilla/uniffi-rs/tags
- [U3] https://github.com/mozilla/uniffi-rs/blob/main/docs/manual/src/foreign_traits.md
- [U4] https://github.com/mozilla/uniffi-rs/blob/main/docs/manual/src/futures.md

Compose Multiplatform und Gobley:
- [C1] https://blog.jetbrains.com/kotlin/2025/05/compose-multiplatform-1-8-0-released-compose-multiplatform-for-ios-is-stable-and-production-ready/
- [C2] https://github.com/JetBrains/compose-multiplatform/releases
- [G1] https://github.com/gobley/gobley

Crux:
- [X1] https://github.com/redbadger/crux/blob/master/README.md
- [X2] https://github.com/redbadger/crux/releases

Rust-Tooling für Android:
- [RS1] https://github.com/rusqlite/rusqlite/blob/master/README.md
- [ND1] https://github.com/bbqsrc/cargo-ndk
- [RV1] https://github.com/rustls/rustls-platform-verifier

Native-Widget-Frameworks:
- [N1] https://github.com/longbridge/nsh/blob/main/README.md
- [N2] https://github.com/longbridge/nsh/releases
- [E1] https://github.com/expo/exact/blob/main/README.md

Discord:
- [DC1] https://x.com/svishnevskiy/status/2118081920194203799 (Primärquelle; ohne Login nicht abrufbar)
- [DC2] https://www.infoq.com/news/2026/11/discord-rust-rewrite-gpui-zed/
- [DC3] https://www.heise.de/en/news/Discord-is-rebuilding-in-Rust-leaving-React-and-Electron-behind-11516113.html
- [DC4] https://www.dexerto.com/entertainment/discord-reveals-major-app-overhaul-that-could-make-it-faster-and-use-less-ram-3425203/
