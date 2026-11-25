# Mobile: Rust-Core + native UI (Android-Prototyp)

Frage: Trägt „Rust-Core + native UI“ für Mobile? Ergebnis des Android-Prototyps (`apps/android`): ja.
Derselbe `quest-store` läuft unverändert auf Android, offline-first und mit Live-Sync.

## Schichten

```
Jetpack Compose (apps/android)          Screens, StateFlow, Threading der Plattform
  └─ ViewModels → CoreHolder (Kotlin)  ein Core pro Prozess (QuestApplication), StateFlow, Ticker
      └─ uniffi.quest_mobile (generiert) Kotlin-Bindings, JNA
          └─ quest-mobile (Rust, cdylib) QuestCore + UI-Records (HomeData, HomeTask, QuestChip, SyncState)
              ├─ quest-view              Präsentationslogik (pur), geteilt mit dem Desktop
              └─ quest-store             SQLite, Outbox, WebSocket-Sync
                  └─ quest-domain        Policy: Commands, Snapshot, Quest Hours (unverändert)
```

- `quest-mobile` reicht keine Domain-Typen nach außen: Ids als Strings, Stunden als fertige Labels
  ("3.6h"), Farben als Hex, Status als flaches Enum. Die Abbildung (`home.rs`, `commands.rs`,
  `status.rs`) ist pur und mit eingefrorener Zeit getestet.
- Zeit kommt von außen: Jeder Aufruf, der „jetzt“ braucht, bekommt `Moment { epoch_ms, time_zone }`
  (IANA-Name aus `ZoneId.systemDefault()`). Kalender und Mitternacht rechnet Rust mit `chrono-tz`.
  Unbekannte Zone → `MobileError.InvalidTimeZone`, `epoch_ms` außerhalb des Bereichs → `InvalidMoment`.
- `Store` ist `Send`, aber nicht `Sync` (er hält einen `mpsc::Receiver`); UniFFI-Objekte müssen
  `Sync` sein. `QuestCore` hält ihn deshalb in einem `Mutex`. Alle Store-Methoden sind
  speicherbasiert, der Lock ist kurz.

## Threading und Callback

- `QuestCore.open` macht Datei-IO (SQLite öffnen, laden) → `Dispatchers.IO`.
- `home`, `addTask`, `toggleTask`, `signIn`, `syncState` arbeiten nur im Speicher; Persistenz und
  Netzwerk laufen im Worker-Thread des Stores. Damit ist kein IO auf dem Main-Thread.
- `ChangeListener.onChange` (UniFFI-Callback-Interface, `with_foreign`) wird vom Store-Worker-Thread
  gerufen. Er ruft dort nicht in den Core zurück (der Store-Mutex ist nicht reentrant, siehe Phase 0),
  sondern markiert den Zustand nur als veraltet. Compose sammelt den `StateFlow` auf dem Main-Thread. Kein Polling.
- Der Core lebt so lange wie der Prozess (`QuestApplication`); `close()` wird nicht gerufen. Drop wartet
  bis 2 s auf ausstehende Writes; beim Wechsel in den Hintergrund ruft die App `flush` (Phase 6).

## quest-view

- Pure Crate mit der Präsentationslogik, die heute in `quest-app` steckte und kein Rune braucht:
  `format`, `timescale`, `log_work`, `entry_editor`, `undo`, `plan_strip`, `quests`, `palette`, `menu` und
  `nav` (`View`). Kein IO, keine Uhr: Zeit wird übergeben.
- `quest-app` bindet die Module per `use quest_view::{…}` ein, Verhalten unverändert (Snapshots
  byte-identisch). `quest-mobile` baut daraus ab Phase 1 seine UI-Records; Kotlin rechnet keine Labels.

## Markdown-Notizen und Checkliste (R4)

- Notizquelle bleibt `TaskDetail.description` (ein synchronisiertes Markdown-Feld). Speichern über
  `QuestCore.setTaskDescription`, nach Eingaben entprellt und beim Verlassen des Editors sofort.
- `parseMarkdown(text): List<MarkdownBlock>`: `Heading(level, content)` (1–3),
  `Paragraph(content)`, `ItemList(start, items)` (ohne `start`: ungeordnet),
  `Quote(blocks)`, `Code(text)`. `MarkdownListItem(marker, blocks)` kann einen
  `MarkdownTaskMarker(checked, offset)` haben. `MarkdownInline(text, bold, italic, kind)`
  hat `MarkdownInlineKind.Text`, `Code` oder `Link(url)`.
  Als native Elemente zeichnen: HTML ist Klartext; nur `http://`, `https://` und `mailto:`
  kommen als Link zurück. Keine WebView oder Interpretation als HTML.
- `toggleTaskMarker(text, offset): String` erhält den **UTF-8-Byteoffset** (`ULong`) aus dem
  Modell, keinen Kotlin-Stringindex. Nur einen Marker aus der aktuellen Notizfassung anwenden.
  Die Funktion ändert die Quelle, nicht den Store; anschließend als Notiz speichern.
  Markdown-Aufgabenpunkte zählen nicht zur strukturierten Checkliste.
- `TaskDetail.checklist` bzw. `QuestCore.checklist(taskId)` liefert sortierte
  `ChecklistRow(id, text, done)`. `ListRow.checklistProgress` und
  `QuestCore.checklistProgress(taskId)` liefern `ChecklistProgress(done, total)` (oder
  `null` ohne Punkte). Die Zeile kann daraus etwa „2/5“ anzeigen.
- `addChecklistItem(taskId, text, position)` fügt einen beschrifteten Punkt ein.
  `insertChecklistItemAfter(itemId)` legt einen leeren Punkt direkt darunter an.
  Beide liefern die ID des neuen Punkts. `renameChecklistItem(itemId, text)` speichert
  eine nichtleere Beschriftung. `moveChecklistItem(itemId, position)` verschiebt den
  Punkt innerhalb seiner Liste; `ChecklistPosition` ist `Top`, `Bottom` oder
  `Before(itemId)`.
- `setChecklistItemDoneWithUndo(itemId, done, now)` setzt den Haken mit Undo. Die ältere
  Variante `setChecklistItemDone(itemId, done)` setzt ihn ohne Undo.
  `deleteChecklistItem(itemId, now)` löscht weich und ist rückgängig machbar.
  Beides mit Undo nutzt denselben Slot wie Task-Aktionen; `QuestCore.undo(now)` nimmt
  die letzte Aktion zurück.

## Sync until idle

- `Store::sync_until_idle(timeout) -> SyncOutcome` (`Idle` | `Offline` | `TimedOut`) blockiert, bis die
  Outbox leer und der Catch-up fertig ist (`Live { pending: 0 }`). `Offline` heißt: Sync kommt gerade
  nicht weiter (abgemeldet, Fehler oder offline). Offline zählt erst, wenn auch ein Wiederholungsversuch
  gescheitert ist: ein einzelner Abbruch oder ein gescheiterter erster Versuch (`attempt` 1, Retry nach
  ~250–500 ms) wird abgewartet, damit ein kurzer Blip den Hintergrund-Flush nicht abbricht. Ohne Server
  kommt `Offline` dadurch etwa eine Sekunde später. Kein Polling: der Worker signalisiert eine Condvar.
  Ein nicht darstellbarer `timeout` (z. B. `Duration::MAX`) wartet ohne Frist.
- Gedacht für den Hintergrund-Sync (WorkManager, Phase 6); nie auf dem UI-Thread. Direkt nach
  `sign_in` kann der Status noch `SignedOut` sein: erst den Sign-in abwarten.

## Ticker

- Kotlin rechnet keine Zeiten. Es ruft `tick(now)` beim Öffnen und danach in einer Schleife:
  `delay(nextWakeMs - nowMs)`, dann wieder `tick(now)`, solange die App im Vordergrund ist.
  `nextWakeMs(now)` liefert dieselbe Weckzeit ohne Seiteneffekt.
- Die Weckzeit liegt 5 ms hinter der Grenze (wie Desktop `clock::WAKE_SLACK`): ein Wecker, der einen
  Hauch zu früh kommt, fände nichts Neues und hätte sofort wieder `delay(≈0)`.
- Regel wie Desktop (`State::refresh`): nächste volle Sekunde, wenn ein Focus-Timer läuft, sonst nächste
  volle Minute. Die nächste Mitternacht in `Moment.time_zone` ist immer eine Weckzeit (`chrono-tz`; bei
  einer Zeitumstellung über Mitternacht, z. B. America/Santiago, beginnt der Tag am Sprung).
- `tick` führt `GenerateDueTasks` aus, wenn sich die UTC-Minute oder der lokale Tag geändert hat oder
  der Snapshot seit dem letzten Lauf ein anderer ist. Während `Connecting` wartet es (wie Desktop),
  damit ein neues Gerät keine Vorkommen erzeugt, die der Server gleich liefert.
- `onChange` kommt nur, wenn sich Daten oder Sync-Status ändern. Nach jedem `tick` liest Kotlin `home`
  deshalb selbst neu (Tageswechsel, laufende Stunden).
- Pur und mit eingefrorener Zeit getestet: `ticker.rs` / `ticker_tests.rs` (Minute, Sekunde, Mitternacht,
  DST).

## HTTPS

- Anmeldung (`reqwest`, Feature `rustls-no-provider`) und Sync-Socket (`tokio-tungstenite`, wss) nutzen dieselbe
  rustls-Konfiguration (`quest-store/src/remote.rs` `tls_config`): Krypto aus `ring` (seit Phase 7, statt
  aws-lc-rs), Zertifikate prüft `rustls-platform-verifier` 0.7. So gelten Nutzer- und Firmen-CAs für beides.
  Auf Android fragt der Verifier per JNI den Trust Manager.
  Dafür braucht er einmal pro Prozess JVM und `Context`, sonst panict die erste HTTPS-Prüfung. Er braucht also
  dieselbe Initialisierung. `webpki-roots` ist nicht mehr im Spiel.
- Einstieg: `app.quest.android.core.RustTls.init(context)` (Kotlin, `external fun`). Die Application ruft
  ihn vor `QuestCore.open`. Rust: `crates/quest-mobile/src/android_tls.rs` (nur
  `cfg(target_os = "android")`) → `rustls_platform_verifier::android::init_with_env`. JNI-Fehler und Panics
  werden zu einer Java-`RuntimeException`.
- `unsafe_code` bleibt workspace-weit `forbid`. Das Export-Symbol `Java_app_quest_android_core_RustTls_init`
  erzeugt `jni::jni_mangle`, so wie UniFFI seine Symbole. Eigener `unsafe`-Code ist nicht nötig.
- Kotlin-Teil des Verifiers: Maven-Artefakt `org.rustls:rustls-platform-verifier`. Upstream liefert es aus
  einem Maven-Repo im GitHub-Repo (Branch `maven-archive`). Die Crate `rustls-platform-verifier-android`
  0.2 enthält es nicht mehr, deshalb gibt es kein lokales Repo aus der Crate-Quelle. Das Repo ist in
  `settings.gradle.kts` auf die Gruppe `org.rustls` beschränkt. Die Version liest
  `app/build.gradle.kts` aus `Cargo.lock` (Eintrag `rustls-platform-verifier-android`), denn Rust- und
  Kotlin-Teil müssen gleich sein. Bei R8/minify:
  `-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }`.
- URL-Regel (`server_url.rs`, gilt auch für Rust-Sockets, die die Network-Security-Config umgehen):
  `https` immer; `http` nur für `localhost`, `127.0.0.0/8`, `::1` und, nur mit `UrlPolicy::Development`
  (Debug-Build), `10.0.2.2`: Auf echten Geräten ist das eine gewöhnliche LAN-Adresse (seit Phase 7). Sonst
  `MobileError.InsecureServerUrl`. Benutzername/Passwort in der URL → `MobileError.ServerUrlWithCredentials`
  (ohne den Wert, damit das Passwort nicht in Meldungen landet). `sign_in` erzwingt die Regel,
  `checkServerUrl(url, policy)` prüft vorab im UI. HTTP-Redirects folgt `quest-store` nicht: Eine 3xx-Antwort
  wird zu `StoreError::InvalidServerUrl` („redirects to …; enter the final address“), damit ein Redirect die
  Regel nicht umgeht und das Passwort nicht erneut sendet.
- Panics: Jede exportierte Methode mit `Result` läuft in `guard` (`catch_unwind`) → `MobileError.Internal`.
  Methoden ohne `Result` (`settings`, `syncState`, `account`, `signOut`) sind reine Speicherzugriffe. Falls
  sie doch panicen, fängt UniFFI das ab, und Kotlin bekommt eine `InternalException`.

## Android-Fundament (Phase 0)

- **Ein Core pro Prozess:** `QuestApplication` ruft `RustTls.init`, dann öffnet `core/CoreHolder` den Core auf
  `Dispatchers.IO` und hält `state: StateFlow<CoreState>` (`Opening` / `Ready` / `Failed`). `onChange`, jede
  Aktion und jeder `tick` senden nur ein Signal in einen conflated Channel. Eine einzige Leser-Coroutine liest
  danach `home`, `syncState`, `account`, `settings` und ist der einzige Schreiber von `state`. So ruft der
  Worker-Callback nie in den Core zurück (Deadlock auf dem nicht-reentranten Store-Mutex, wenn `send` synchron
  benachrichtigt), und ein älteres Lesen überschreibt kein neueres. Aktionen laufen über
  `perform { core, now -> … }` und liefern `Outcome` (`Done` / `Failed(message)` / `NotReady`). `state` folgt
  kurz danach. Jede Exception aus dem Core (auch UniFFI-`InternalException` nach einem Panic) wird zu
  `Outcome.Failed`, einer Notice oder `CoreState.Failed`, nicht zum Absturz.
  Hintergrundfehler (Tick, Lesen) kommen als `notices` und erscheinen als Snackbar.
- **Kein Backup der Core-Daten:** `allowBackup="false"` und `res/xml/data_extraction_rules.xml` schließen
  `files/quest` von Cloud-Backup und Gerätetransfer aus. Die Datenbank enthält die `NodeId` und das Session-Token.
  Eine Kopie auf einem zweiten Gerät würde als derselbe Knoten synchronisieren (HLC-Kollisionen) und die Session
  mitnehmen. Ein neues Gerät meldet sich an und lädt die Daten vom Server.
- **Ticker:** `ProcessLifecycleOwner` schaltet `setForeground(true/false)`; die Schleife wartet
  `tick(now) - now` (seit Phase 1 vorzeitig geweckt, sobald eine Aktion den Core ändert). Kotlin rechnet nur
  diese Differenz. Schlägt ein Tick fehl, neuer Versuch nach
  einer Minute.
- **Testbarkeit:** `core/QuestCoreApi` ist die Kotlin-Grenze zum Core (`RustQuestCore` ist die einzige
  native Implementierung). ViewModel- und Holder-Tests laufen mit einem `FakeCore` auf der JVM, ohne `.so`.
  `ServerUrlPolicy` (Vorprüfung über `checkServerUrl`) und `Clock` sind ebenfalls austauschbar.
- **Darstellung** (System/Hell/Dunkel) ist kein Domain-Wert: nur lokal in DataStore Preferences
  (`settings/AppearanceStore.kt`). Das Wochenziel ist synchronisiert (`setWeeklyTarget`, Rust prüft 1–168 h).
- **Theme:** Der Gradle-Task `generateQuestColors` erzeugt `QuestColors.kt` (hell und dunkel) aus
  `crates/quest-app/assets/theme.json`. Typ-Skala und Abstände stehen in `ui/theme/QuestType.kt`, übertragen
  aus `theme.rs`. Schrift ist Inter wie am Desktop (`res/font`, Lizenz `apps/android/INTER-LICENSE.txt`).
  `QuestTheme(dark)` setzt das Material-3-Farbschema und die Typografie aus diesen Tokens.
- **Komponenten** (`ui/components`): `TaskCard`, `QuestChip`, `HoursHeader`, `SectionHeader`,
  `QuestBottomSheet`/`QuestSheetContent`, `UndoSnackbar` und `showUndo`, `SyncPill`.
- **Navigation:** `NavigationSuiteScaffold` wählt nach Window Size Class Bottom Bar (compact) oder
  Navigation Rail (medium/expanded). Home, Plan, Calendar, Quests, Review; Suche und Account oben rechts.
  Account → Settings. Plan/Calendar/Quests/Review/Suche sind Platzhalter.
- **Screenshot-Tests:** Roborazzi auf Robolectric (`src/test/.../screenshots`), jede Szene hell/dunkel ×
  Phone (411×914 dp) / Tablet (1280×800 dp). Referenzbilder in `app/src/test/screenshots`, neu aufnehmen mit
  `./gradlew recordRoborazziDebug`, prüfen mit `verifyRoborazziDebug`. Robolectric läuft dabei mit SDK 36:
  das SDK-37-Image zeichnete manche Compose-Frames leer.
- minSdk 31, targetSdk 37; `cargo ndk --platform 31`.

## Phase 1 (Rust): Home und Task

API-Vertrag mit allen Records: `quest-mobile` (`types.rs`, `task_types.rs`); Kotlin rechnet keine Labels,
Dauern, Validierungen oder Positionen.

- **Home:** `home(now, HomeQuery { quest_filter, quest_pick })` liefert `quest_work` und `other` (Desktop
  `section_of`; offene zuerst), Quest Hours heute/Woche, `weekly_target`, Fortschritt, Filter-Chip, den Chip für
  neue Quest-Arbeit (`add_quest`) und den Undo-Toast. Filter, Pick, „Other“ sichtbar und Oben/Unten sind
  UI-Zustand (Desktop hält sie auch nur im Speicher). `add_task(title, section, position, query, now)` wählt
  die Quest nach `quest_view::home::new_task_quest` (Filter → Pick → zuletzt genutzte → erste aktive).
- **Karte:** `HomeTask.tracked` wie die Desktop-Karte (`format::timer`), `timer_running`, `can_start_timer`;
  `start_timer` (stoppt einen anderen laufenden Timer), `stop_timer`, `rename_task` (Regel aus
  `quest_view::home::rename`: leer/unverändert speichert nichts), `toggle_task`, `delete_task`.
- **Undo:** `undo.rs` (`MobileUndo`) hält den Desktop-`UndoState` für Löschungen und ergänzt mobil
  „Completed/Reopened “…”“ mit Undo für den Haken/Swipe. `undo(now)`, `dismiss_toast(now)`; `tick` lässt den
  Toast nach 6 s verschwinden, `next_wake_ms` weckt dafür rechtzeitig.
- **Task-Sheet:** `task_detail` (Titel, Beschreibung, Quest + Optionen, Datum ISO + Label, Wiederholung,
  Zeithistorie neueste zuerst mit Summe) und Setter `set_task_description`, `set_task_quest`, `set_task_day`,
  `set_task_repeat`, `clear_task_repeat`. Der Repeat-Dialog ist `RepeatDraft` (alle Felder des Desktop-Dialogs);
  Validierung und Texte stammen aus `quest_view::repeat` (`RepeatForm::rule`), live über `check_repeat` und
  `toggle_repeat_weekday`. Wiederholung wirkt auf die Serie (`series_of`); fällige Vorkommen entstehen sofort.
- **Tagesraster:** `day_grid(day, now)` (`day_grid.rs`): Sitzungen (laufende bis jetzt), geplante Blöcke, Lanes
  bei Überlappung (`quest_view::timescale::lanes`), Now-Minute, Stundenmarken; 00:00–24:00, beliebiger Tag
  (Grundlage für Calendar).
- **Umzug nach quest-view** (Desktop nutzt es von dort, Snapshots byte-identisch): `home` (`new_task_quest`,
  `rename`, `one_line`) und `repeat` (`RepeatForm`, `series_of`, `cadence`, `summary`, `toggle_weekday`).
- **Fehler:** neu `UnknownTask`, `TaskIsDone`, `EmptyTitle`, `InvalidDate`, `InvalidRepeat { reason }`.
- **desktop_peer:** Befehle für alle neuen Aktionen (rename, describe, delete/restore, start/stop, move, repeat,
  norepeat, assign, show), siehe Kopf von `examples/desktop_peer.rs`.

## Phase 2 (Rust): Focus

- **Umzug nach quest-view:** `focus::target(snapshot, focus_task)` ist die Desktop-Regel aus
  `quest-app/src/state/focus.rs` (laufende Aufgabe, sonst die zuletzt gestartete/gewählte offene, sonst die offene,
  an der zuletzt gearbeitet wurde). Der Desktop ruft sie von dort; Snapshots byte-identisch.
- **API** (`core_focus.rs`, `focus.rs`): `focus(now, focus_task) -> FocusView` (`Session` mit Titel, Beschreibung,
  Quest, Kopf-Label „FOCUS · WORK“, Timer über alle Sitzungen der Aufgabe, RUNNING/PAUSED; sonst `Pick` mit den
  offenen Aufgaben von heute). `focus_task` hält Kotlin (zuletzt gestartet/gewählt); unbekannte Ids werden ignoriert.
  `running_session(now)` für die Notification mit `chronometer_base_ms` = jetzt − erfasste Zeit der Aufgabe, damit
  der Chronometer denselben Wert zeigt wie der Focus-Timer. `complete_task(task_id, now)` = Desktop-Complete:
  erledigt eine offene Aufgabe (stoppt ihren Timer, Undo-Snackbar wie der Haken), eine erledigte bleibt.
  Pause/Fortsetzen, Titel und Beschreibung nutzen die Phase-1-Methoden.
- **Logged time:** Summe und Einträge im Task-Sheet einheitlich mit `quest_view::entry_editor::duration_label`
  („1h 26m“, „1m“, „1m total“) statt „0h total“ neben „01:26“.

## Phase 3 (Rust): Plan

- **Umzug nach quest-view:** `reorder::nudge` (Desktop ⌥↑/⌥↓, `dnd.rs` re-exportiert es) und
  `palette::move_days` (die Tage von „Move to…“, `move_to` nutzt sie). Desktop unverändert.
- **Fenster** (`plan.rs`, pur): `plan_window(first_visible, visible_days, built, now)` liefert ganze Wochen mit
  14 Tagen Überhang je Seite. Ein gebautes Fenster hält, solange 3 Tage jenseits jedes sichtbaren Rands gebaut sind
  (Hysterese wie `plan_strip::next_window`). Höchstens 3·366 Tage um heute. `plan_edge_speed` ist
  `plan_strip::edge_speed` (80 dp Zone, bis 1400 dp/s).
- **Tage:** `plan(window, quest_filter, now)` listet je Tag QUEST WORK und OTHER wie die Desktop-Spalte
  (`drag::list_tasks`): nach `sort_key`, offen vor erledigt, Filter nur auf QUEST WORK, dazu die
  **Vorschau-Wiederholungen** (`planned_occurrences`).
- **Vorschau-Wiederholungen bearbeiten:** Der Core merkt sich das zuletzt gelesene Fenster, den Zeitpunkt und den
  Filter. `task()` findet darüber auch Vorschauen. `dispatch` hüllt Aufgaben-Edits wie der Desktop (`plan_edit`) in
  `EditPlan`: Die Vorschau wird erst gespeichert, Verschiebungen auf einen Tag sortieren zwischen dessen Vorschauen.
  Alle Karten- und Sheet-Aktionen funktionieren so mit Vorschau-ids. `leave_plan()` vergisst das (Desktop: nur in
  der Plan-Ansicht).
- **Erledigen:** `toggle_task` sendet `CompleteTaskFromPlan`; eine künftige Wiederholung wandert auf heute. Undo
  öffnet sie wieder und legt sie ans Ende ihres alten Tages (mobile Ergänzung, der Desktop hat dieses Undo nicht).
- **Verschieben:** `plan_move_task(task, day, before)` = Desktop-Drop (`MoveTask` mit `Before`/`Bottom`). Eine offene
  Karte vor einer erledigten landet hinter der letzten offenen (Anzeige offen vor erledigt). Vor sich selbst = No-op.
  `plan_nudge_task` (eine Stelle im Abschnitt, wie angezeigt gefiltert), `plan_shift_task` (Vortag/Folgetag, ans
  Ende), `move_targets` (Palette „Move to…“), `plan_add_task` (ohne Quest, wie das Add-Feld der Desktop-Spalte).

## Phase 4 (Rust): Calendar, Log work, Zeiteinträge

Vertrag: `calendar_types.rs` (Records), `core_calendar.rs` (Methoden). Kotlin rechnet Minuten nur in dp um und
übergibt die ungesnappte Minute unter dem Finger; Tage, Seiten, Snapping, Clamping, Dauern, Validierung und Quest
kommen aus Rust.

- **Seiten** (`calendar.rs`, pur): `calendar(CalendarQuery { first_day, day_count }, now)`. 1 Tag (compact), 3
  (medium) oder eine Woche ab Montag (expanded); ohne `first_day` die Seite mit heute (3 Tage: heute in der Mitte).
  `previous_first_day`/`next_first_day` blättern um `day_count` Tage. Je Tag `DayGrid`, Wochentag, Tagessumme aller
  erfassten Arbeit (Desktop); Kopf mit Quest Hours der Woche des mittleren Tags gegen das Wochenziel, `scroll_minute`
  (Beginn der aktuellen Stunde), Monat und Undo-Toast.
- **Blöcke** (`day_grid.rs`): `GridBlock` sagt, welche Drags er nimmt (`can_move`, `start_handle`, `end_handle`,
  Desktop `timegrid::ends_on`): laufende Sitzung nur Start-Griff, ein über Mitternacht geschnittenes Segment nur den
  Griff seines Endes auf diesem Tag, geplante Blöcke alles (der Start-Griff ist mobil neu). `target_id` (Eintrag
  bzw. Aufgabe) und `a11y_label` für TalkBack.
- **Drags** (`grid_drag.rs`, pur): `grid_drag_preview` (Geist) und `grid_drop` folgen `dnd::drop::place` auf einem
  Raster von 00:00 bis 24:00: Viertelstunden, mindestens 15 min, innerhalb des Tages, Greif-Versatz beim Verschieben.
  Nur `Move` wechselt den Tag. Leere Fläche liefert einen `LogWorkDraft` und speichert nichts (Desktop
  `Placement::NewEntry`); ein Zeitraum bis 24:00 wird im Formular 23:59. Ein Drop an derselben Stelle speichert
  nichts; eine Kante, die auf ihrer Minute bleibt, behält ihre Sekunden. Ein geplanter Block über Mitternacht
  behält beim Verschieben seine ganze Länge und hat keinen End-Griff.
- **Undo:** Löschen eines Eintrags wie am Desktop („Deleted “…”“). Mobil zusätzlich: ein Drag zeigt „Moved “…”“ bzw.
  „Changed “…”“; Undo setzt die alten Zeiten (Sekunden genau) bzw. den alten Block zurück (`undo.rs`: `PutBack`).
  Wurde ein laufender Timer inzwischen gestoppt, behält Undo dessen Ende. Ein Block, der auf einen anderen Tag
  verschoben wurde, kommt ans Ende seines alten Tages (`ScheduleTask`).
- **Log work** (`work_forms.rs`): `log_work_now` (die gerade beendete Viertelstunde), `check_log_work` (Labels,
  Dauer, Quest der Aufgabe bzw. gewählte Quest einer neuen Aufgabe, Fehler am Feld als `FormField`: Task, Date,
  EndDate, Start, End) und `log_work`. Validierung und Meldungen sind `quest_view::log_work::decide`. `task_choices(query)` ist die Desktop-Combobox (ohne Suche alle,
  offene zuerst; sonst bis 5 Treffer; „Create “…”“).
- **Entry-Editor:** `entry_draft`, `check_entry`, `save_entry`, `delete_entry`. Zeiten über
  `quest_view::entry_editor::times` (Sekunden bleiben bei unveränderter Minute, Enddatum für über Mitternacht).
  **Kein Quest-Feld:** gespeichert wird die Quest der Aufgabe, ohne Aufgabe keine (`EditEntry.quest_id`).
- **Zeitzone:** `Now::instant` liest Ortszeit; eine übersprungene Zeit (Sommerzeit) liest sich eine Stunde später.
  Sitzungen liegen im Raster in Wanduhr-Minuten wie Blöcke und Drops.
- **Fehler:** neu `UnknownEntry`, `InvalidForm { reason }`.
- **desktop_peer:** `log`, `retime`, `unlog`, `block`; `show` listet Sitzungen und Blöcke.

## Phase 5 (Rust): Quests, Review, Archiv, Suche

Vertrag: `quest_types.rs`, `review_types.rs`, `search_types.rs` (Records), `core_quests.rs`, `core_search.rs`
(Methoden). Alle Stunden (auch Quest Hours) und Review-Zahlen kommen aus Rust, im Desktop-Format („0.5h“).

- **Domain:** neues Feld `Quest.description` (LWW-Feld „description“) mit `SetQuestDescription`;
  `WeeklyReview.by_quest_per_day` (Quest-Arbeit je Tag nach Quest, in der Reihenfolge von `by_quest`).
- **quest-view:** `quests` (pur aus `quest-app` verschoben): `new_quest_color` (erste freie Farbe),
  `quest_ms`/`total_ms`, `task_groups` (Tage, Backlog, Done), `day_label`, `color_name`. Desktop unverändert.
- **Quests** (`quests.rs`): `quests(now)` mit Active/Completed, Stunden der Woche und gesamt, offene Aufgaben,
  8-Wochen-Trend, letzte Meilensteine, a11y-Label, Archiv-Zähler. `quest_detail` mit Farbauswahl, Gruppen als
  `HomeTask`, Meilensteinen (neueste zuerst). „Complete quest“ = Status Archived, „Reopen“ = Active.
  Methoden: `create_quest`, `rename_quest`, `set_quest_description`, `set_quest_hue`, `complete_quest`,
  `reopen_quest`, `delete_quest` (Undo), `add_quest_task` (heute oder Backlog), `add_milestone` (heute erreicht),
  `rename_milestone`, `set_milestone_date` (nicht in der Zukunft), `delete_milestone` (Undo).
  Überholt durch Phase 17 (siehe dort): kein Trend, kein Archiv-Zähler, Status Completed, keine Meilensteinpflege.
- **Review** (`review.rs`): `review(week, now)` über `WeeklyReview::build`: Quest Hours gegen Ziel, Tagesschnitt,
  Anteil, gestapelte Tagesbalken (Bruchteile vom größten Tag, mindestens 1 h), Stunden je Quest, Quest work vs
  Other, 8 Wochen Verlauf; jede Grafik mit fertigem a11y-Text.
- **Archiv** (`archive.rs`): `archive(now)` gruppiert den Papierkorb (Quests, Aufgaben, Meilensteine, Sitzungen);
  `restore(kind, id, now)` wie am Desktop (räumt den Undo-Toast ab) (eine Quest bringt ihre Kinder mit).
- **Suche** (`search.rs`): `search(mode, query, now)` nimmt `palette::entries` unverändert (Ranking, max. 12),
  Tastenkürzel werden zu Wörtern; Untermodi `MoveTo`/`ScheduleAt`. `run_search(token, now)` führt aus und sagt
  der UI per `SearchOutcome`, wohin (Create quest → Quest-Seite, Focus startet den Timer).
- **P4-Nacharbeit:** `check_log_work` bietet die Quest-Wahl nur für eine neue Aufgabe an.
- **Fehler:** neu `UnknownQuest`, `UnknownMilestone`.
- **desktop_peer:** `newquest`, `qcomplete`, `qreopen`, `qrename`, `qdescribe`, `qcolor`, `qdelete`, `milestone`;
  `show` listet Quests.

## Phase 6 (Rust): Plattform

- **Flush:** `Store::flush(timeout) -> bool` schickt `Request::Flush` an den Worker. Anfragen laufen der Reihe nach,
  also sind alle früheren `Persist` erledigt, wenn er antwortet: true = nichts Ungespeichertes mehr, false = Zeit
  um oder Schreibfehler (die Änderungen bleiben im Speicher, Retry wie bisher). Damit verliert ein Prozess-Kill
  nach `onStop` nichts mehr (Befund aus Phase 0).
- **`Store::waiter() -> SyncWaiter`:** Klon von `Shared` und Request-Sender. `QuestCore` hält ihn neben dem
  Store-Mutex, damit `sync_until_idle` und `flush` blockieren, ohne den Lock zu halten; UI-Aufrufe laufen weiter.
- **API** (`core_platform.rs`): `sync_until_idle(timeout_ms) -> SyncOutcome` (`Idle`/`Offline`/`TimedOut`),
  `flush(timeout_ms) -> bool`, `next_day_ms(now)` (Beginn des nächsten lokalen Tags + `WAKE_SLACK`, für den
  Widget-Alarm), `quick_add_form(shared_text, now)`, `quick_add(title, description, quest_id, day, now)`; frei `picked_day(iso)` (Label zum Tag aus dem DatePicker).
- **Quick-Add** (`quick_add.rs`, pur): erste nichtleere Zeile des geteilten Texts = Titel, Rest = Beschreibung.
  Geteilter Text höchstens 10 000 Zeichen. Vorauswahl Quest = `quest_view::home::new_task_quest(snapshot, None, None)` (zuletzt genutzte, sonst erste
  aktive), Tag = heute. Anlegen am Ende des Tags (`Position::Bottom`, wie der Desktop-Dialog), `day` None = Backlog.
- Widget, „Start timer on last task“ und Pending-Anzeige nutzen vorhandene APIs (`home`, `toggle_task`, `focus`,
  `start_timer`, `sync_state`).

## Was gut ging

- Cross-Compile ohne Anpassungen: `rusqlite` (bundled SQLite), `tokio`, `tokio-tungstenite`
  (webpki-roots), `reqwest` mit rustls/aws-lc-rs bauten mit `cargo ndk` beim ersten Versuch;
  aws-lc-sys braucht kein CMake mehr. Kalter Rust-Build 51 s, `assembleDebug` nach `clean` 24 s.
- UniFFI-Proc-Macros (0.32) ohne UDL; Library-Mode-Bindgen liest die Metadaten direkt aus der
  Android-.so. Das Bindgen-Binary steckt hinter dem Feature `bindgen`, damit der normale
  Workspace-Build kein clap & Co. baut. `quest-mobile` baut im Workspace ohne NDK (Host-Target).
- Gradle-Integration über die Variant-API (`addGeneratedSourceDirectory`) für `.so` und Kotlin;
  der Cargo-Task hängt automatisch vor dem Kompilieren.
- Sync-Nachweis: Desktop → Emulator und
  Emulator → Desktop live im selben Moment; Offline-Änderungen kommen nach dem Reconnect an
  (hier ~25 s wegen Reconnect-Backoff).

## Was hakte

- **UniFFI-Fehler:** Ein Feld namens `message` in einem Fehler-Enum kollidiert mit
  `Throwable.message` im generierten Kotlin (Compile-Fehler REDECLARATION). Lösung:
  `#[uniffi(flat_error)]`, Kotlin bekommt die `Display`-Meldung.
- **Store nicht `Sync`:** siehe oben, `Mutex` in `quest-mobile` statt Änderung an `quest-store`.
- **TLS auf Android:** `reqwest` (Feature `rustls`) nutzt `rustls-platform-verifier`. Der braucht
  auf Android eine JNI-Initialisierung und eine Kotlin-Komponente aus Maven
  (`rustls-platform-verifier-android`), sonst panict die erste HTTPS-Prüfung. Seit Phase 0 gelöst,
  siehe Abschnitt HTTPS.
- **Network-Security-Config greift nicht für Rust:** Sie regelt nur Java/Android-HTTP-Stacks. Die
  Sockets aus Rust umgehen sie; die Config ist dokumentierend (`src/main`: kein Klartext, `src/debug`: zusätzlich
  10.0.2.2).
  Erzwungen wird die Regel in `quest-mobile` (`server_url.rs`).
- **Größe der .so:** 12,9 MB unstripped (UniFFI braucht die Symbole für Bindgen), 10,0 MB im APK
  nach Strip durch AGP, ~3,9 MB gzip. Großteil: aws-lc (Krypto), SQLite, tokio, reqwest/hyper.
  Hebel: LTO/`opt-level="z"`/`panic=abort` in einem eigenen Profil, `ring` statt aws-lc, nur ein
  HTTP-/TLS-Stack (reqwest und tungstenite bringen je eigenen Ballast). Phase 7: 6,4 MB im APK, siehe
  „Phase 7 (Rust)“.
- **Werkzeuge:** Homebrews `sdkmanager` installiert in sein eigenes SDK-Verzeichnis, nicht in
  `~/Library/Android/sdk`. Aktuelle AndroidX-Bibliotheken verlangen `compileSdk` 37.1+.

## Was iOS (SwiftUI) wiederverwenden kann

- `quest-mobile` komplett: dieselben Records, `QuestCore`, `ChangeListener`, `Moment`. UniFFI erzeugt
  Swift mit `--language swift`; Ziel `aarch64-apple-ios(-sim)` als staticlib/XCFramework.
- Das Muster bleibt: `open` im Hintergrund (`Task.detached`), `onChange` → `@MainActor`-Update
  eines `@Observable`-Modells.
- TLS auf iOS: `rustls-platform-verifier` nutzt dort Security.framework ohne extra Setup.
- Neu zu schreiben ist nur die UI (SwiftUI) und das Build-Skript (cargo + xcodebuild -create-xcframework).

## Phase 1 (Android): Home und Task

Compose-UI auf dem Vertrag oben; Kotlin rechnet keine Zeiten, Stunden oder Validierungen.

- **CoreHolder:** hält die `HomeQuery` (Filter, Pick) und liest damit `home`, dazu `day_grid(home.day_iso)`
  (`CoreState.Ready.todayGrid`). `read { }` für Abfragen ohne Änderungssignal (Task-Detail, Repeat-Entwurf).
  Jede Aktion weckt den Ticker sofort; sonst schlief er nach dem Timer-Start bis zur nächsten Minute und die
  Sekunden standen still.
- **Home** (`ui/home`): Kopfzeile mit Wochenziel, Quest Work / Other (`HomeSections.kt`), Filter-Chips,
  Erfassen mit Quest-Chip (Pick) und Oben/Unten. Ab 600 dp Breite Liste und Tagesraster nebeneinander, darunter
  ein Umschalter Liste/Raster. Eingaben (Entwürfe, Filter, Pick, Ansicht) in `SavedStateHandle`
  (`HomeInputs`); „Other“ sichtbar und Oben/Unten in DataStore (`settings/HomePreferences.kt`).
- **TaskCard** (`ui/components/TaskCard.kt`): Haken, Timer ▶/❚❚, erfasste Dauer, Tippen auf die Karte (auch
  auf den Titel) öffnet das Sheet; dort wird der Titel bearbeitet (seit Phase 14 kein Inline-Umbenennen mehr).
  `SwipeTaskCard`: rechts erledigen/öffnen, links löschen, Undo-Snackbar aus dem Core-Toast; dieselben Aktionen als Accessibility-Custom-Actions.
  Material ruft `confirmValueChange` mehrfach pro Geste auf und stellt einen weggewischten Zustand nach
  Prozess-Tod wieder her. Deshalb setzt die Geste nur ein (nicht gespeichertes) Flag, die Aktion läuft einmal
  pro erreichtem Wert und die Karte schnappt zurück.
- **Task-Sheet** (`ui/task`): `ModalBottomSheet` (nur Expanded, Predictive Back), Titel und Beschreibung
  speichern bei Done, Fokusverlust oder Schließen. Entwürfe gegenüber Sync: Solange ein Entwurf besteht,
  zeigt das Sheet ihn; eine Änderung von einem anderen Gerät wird beim Speichern ohne Hinweis überschrieben
  (last writer wins, wie am Desktop). Ein Lesefehler lässt das Sheet mit seinen Entwürfen offen und zeigt die
  Meldung; nur eine gelöschte Aufgabe (`UnknownTask`) schließt es. Datum über M3-DatePicker (`PickerDays`:
  UTC-Millis ↔ ISO, reiner Plattformadapter; die Datumsfelder der Wiederholung zeigen `RepeatCheck.starts/ends`
  als Label und werden nur über den Picker gesetzt),
  „Move to backlog“, Quest-Auswahl, Zeithistorie, Löschen mit Undo. Die Repeat-Seite (`RepeatEditor.kt`) zeigt
  alle Felder des Desktop-Dialogs; Prüfung, Zusammenfassung, Platzhalter und Meldungen kommen live aus
  `check_repeat` / `toggle_repeat_weekday` (`core/RepeatRules`). Back führt von der Repeat-Seite zurück.
- **Tagesraster** (`ui/components/DayGrid.kt`): wiederverwendbar für einen beliebigen `DayGrid`; skaliert nur
  Minuten auf dp. Sitzungen schraffiert mit Farbstreifen, laufend gestrichelt, geplant als Umriss, Lanes,
  Jetzt-Linie; scrollt anfangs auf zwei Stunden vor jetzt.
- **Qualität:** alle UI-Strings in `res/values/strings.xml` (Englisch wie Desktop, auch die aus Phase 0),
  contentDescription für Icons und Rasterblöcke, Touch-Ziele ≥ 48 dp,
  Edge-to-Edge mit IME-Insets am Inhalt (`QuestScaffold`), Undo-Snackbar als Live-Region, Oben/Unten als
  Radio-Gruppe. Test-Tags sind auch im Sheet-Fenster resource ids.
- **Tests:** ViewModel-Tests mit `FakeCore` (Home, TaskSheet, CoreHolder, PickerDays), Compose-Interaktion auf
  Robolectric (`HomeInteractionTest`: Swipe löst genau eine Aktion aus, Tippen auf den Titel öffnet das Sheet),
  Roborazzi-Szenen hell/
  dunkel × Phone/Tablet (Home Liste+Raster, Compact-Raster, Filter mit Snackbar, Task-Sheet, Repeat Woche,
  Repeat Monat mit Fehler, Karte mit laufendem Timer, Snackbar, Raster).

## Phase 2 (Android): Focus und Foreground-Service

Kotlin liest `focus(now, focus_task)` und `running_session(now)`; Zeiten, Labels und die Wahl der
Focus-Aufgabe kommen aus Rust.

- **Home kompakt:** Auf dem Phone sind ohne Scrollen mindestens drei Karten sichtbar. Der Kopf ist kleiner
  (Datum 20 sp, Liste/Raster als Icon-Button neben dem Datum statt Segmented Button, engere Abstände). Karten
  haben kaum Innenabstand, weil Titel und Steuerzeile schon 48-dp-Ziele sind. Die erfasste Zeit ist immer 48 dp
  hoch, damit die Zeile beim Start/Stopp nicht springt. Erfassen ist ein einzeiliges Feld mit dem Quest-Chip
  vorn und Oben/Unten als Icon-Button hinten (Role.Switch mit stateDescription). Die Filter-Chips stehen in
  einer horizontal scrollenden Zeile (`LazyRow`).
- **Focus** (`ui/focus`): ersetzt das Scaffold, solange es offen ist. Home wird darunter nicht komponiert, so
  erreichen es weder Touches noch TalkBack; sein gespeicherter UI-Zustand bleibt per `SaveableStateHolder`. Sichtbarkeit und `focus_task` liegen im
  SavedStateHandle (`FocusInputs`) und überstehen einen Prozess-Tod. PredictiveBackHandler skaliert den Inhalt.
  Der Kopf zeigt Zurück, Punkt (laufend/pausiert) und `header`. Quest-Chip (`set_task_quest`, Optionen aus
  `home.quests`), Titel (`rename_task`) und Beschreibung sind editierbar. Entwürfe werden bei Fokusverlust,
  Done, Verlassen, Complete und `ON_STOP` gespeichert (Wegwischen aus Recents kann den Prozess beenden). Großer Timer (`TextAutoSize`), RUNNING/PAUSED, Pause/Resume,
  Complete (`complete_task`, danach Home). `FocusView.Pick` startet die gewählte Aufgabe. Zugang über die
  laufende Zeit auf der TaskCard, das Top-Bar-Icon, den Notification-Tap und den Deep Link `quest://focus`
  (`MainActivity` ist `singleTop`; `onNewIntent`). Bildschirm wach über `View.keepScreenOn`; Setting in
  DataStore (`settings/FocusPreferences.kt`), Default an.
- **Service-Entscheidung** (`focus/ServiceDecision.kt`, pur): `decideService(shown, TimerReading)` →
  Nothing/Start/Update/Stop. Start und Stop folgen allein `running_session`. Gezeigt wird Unknown (nach einem
  Prozess-Start; eine Notification von vorher geht, wenn keine Sitzung läuft), None, Running (Service),
  Fallback (einfache Notification, weil Android den FGS-Start verweigert hat) oder Paused. Eine
  Paused-Notification bleibt, solange ihre Aufgabe offen ist (`focus(now, taskId)` liefert eine Session dieser
  Aufgabe) und kein Timer läuft.
- **FocusNotifier** (einer pro Prozess, `QuestApplication`): liest nach jeder Core-Änderung `running_session`
  und setzt die Entscheidung um. Er stoppt den Service nie selbst (`stopService` während eines ausstehenden
  `startForegroundService` bringt die App zum Absturz), sondern bittet einen laufenden Service, sich nach dem
  gezeigten Zustand zu richten (`settle`). Ein Service, der noch startet, richtet sich danach von selbst. Bei
  `ForegroundServiceStartNotAllowedException` (Start aus dem Hintergrund) zeigt er eine einfache Notification
  (Fallback) und startet den Service, sobald die App nach vorn kommt (`appResumed` in `onResume`). `repost()`
  nur nach erteilter Erlaubnis: Android holt eine vorher verworfene FGS-Notification nicht nach, und eine
  weggewischte soll nicht zurückkommen.
- **FocusService:** Typ `specialUse`. Begründung im Manifest: eine vom Nutzer gestartete Stoppuhr, die mit
  anderen Geräten synchron bleibt. `dataSync` (ab Android 15 auf 6 h begrenzt, inhaltlich unpassend) und
  `shortService` (~3 min) passen nicht. Jeder Start geht sofort in den Vordergrund (Platzhalter, falls der Core
  noch öffnet), führt seine Aktion aus, sobald der Core offen ist, und richtet sich dann nach dem Notifier. Nur
  der neueste Start darf stoppen (`stopSelf(startId)`), offene Aktionen gehen vor; so geht ein Resume direkt
  nach Pause nicht verloren. Ongoing-Notification: Kanal LOW, `CATEGORY_STOPWATCH`, Titel, Quest, `setUsesChronometer`
  mit `setWhen(chronometer_base_ms)` (das System zählt), Aktionen Pause/Complete bzw. Resume/Complete über
  `PendingIntent.getForegroundService`. Pause → `STOP_FOREGROUND_DETACH`, die Paused-Notification bleibt
  (wegwischbar); Resume startet den Service neu; Complete oder ein Stopp anderswo entfernen sie.
  `START_NOT_STICKY`. Der Service läuft nach dem Wegwischen aus Recents weiter. Solange er läuft, tickt der
  Core (`CoreHolder.setServiceRunning`).
- **Erlaubnis:** POST_NOTIFICATIONS wird einmal gefragt, beim ersten erfolgreichen Timer-Start (Karte, Pick,
  Resume im Focus); ein Merker liegt in DataStore.
- **Tests:** `ServiceDecisionTest`, `FocusNotifierTest` und `FocusServiceTest` (Robolectric über `FakeCore`, App
  `TestFocusApp` als `FocusHost`: Reihenfolge, verweigerter Start, Kaltstart mit Pause→Resume, Complete, Stopp
  anderswo während des Starts), `FocusViewModelTest`, `QuestAppFocusTest` (Tipps auf Focus erreichen Home
  nicht), `CoreHolderTest` (Ticker mit Service), `HomeInteractionTest` (Focus öffnen, Oben/Unten),
  `SettingsViewModelTest`. Roborazzi hell/dunkel ×
  Phone/Tablet: Focus laufend, pausiert, Pick, Pick leer, Home mit Entwurf im kompakten Feld. „Logged time“
  in Fake und Referenzen im Rust-Format („1h 26m“).

## Plan-Tagesliste (R2c, UniFFI)

Die Android-UI liest `QuestCore.plan_sections(now: Moment) → PlanSections`. Das Modell stammt aus derselben
Abfrage wie auf dem Desktop. `days` enthält 14 Tagesabschnitte ab heute, auch leere. Für den ersten Abschnitt
rendert Android `task_list(Today, now)` inklusive Datumsmarkern, Verweildauer nach dem Abhaken und „n done“.
`later` ist eine Liste von `PlanMonthSection { start, partial, days }`; `start` ist der Monatserste und
`partial` markiert „Rest of …“. `not_planned` enthält offene Quest-Tasks ohne Plan-Datum (UI initial zu).

- `PlanDaySection { date, tasks, due }`: `tasks` sind `HomeTask`-Zeilen; `due` enthält
  `PlanDueMarker { task_id, title }`. Zeigt „⚑ Due: …“ an dem Datum, wenn der Task dort keine eigene Zeile hat.
- `weeks: Vec<PlanWeek>`, mit je sieben `PlanWeekDay { date, has_tasks, jump_enabled }` (Mo–So). Zeige den
  Punkt bei `has_tasks` und erlaube den Sprung nur bei `jump_enabled`.
- `QuestCore.place_task(task_id, list: PlanTaskList, before_task_id, now)`: Position vor dem Anker, ohne
  Anker am Ende. `PlanTaskList` ist `Today`, `Day { date }`, `Inbox` oder `Quest { quest_id }`.
  Innerhalb von Today bleibt ein liegengebliebener Plantag erhalten. Ein Drop auf einen anderen Tag setzt
  das Plan-Datum samt gekoppelter Deadline und Verschiebe-Zählung. Aus „Not planned“ bezeichnet `Day { date }`
  auch heute ausdrücklich als neues Plan-Datum. Reines Sortieren ist still rückgängig machbar; Umplanen zeigt Undo.
- Datum ist jeweils `YYYY-MM-DD`, IDs sind Strings. Für das Tages-Plus `add_quick_draft` mit
  `QuickFields.planned_on = date` verwenden. `plan_sections` registriert die Repeat-Vorschauen zur Bearbeitung;
  beim Verlassen `leave_plan()` aufrufen.

Die ältere Board-API bleibt in UniFFI verfügbar, wird aber in Android nicht mehr verwendet.

## Phase 3 (Android): Plan (historisch)

Kotlin liest `plan(window, quest_filter, now)`; Fenster (`plan_window`), Rand-Geschwindigkeit
(`plan_edge_speed`), Labels, Ziele für „Move to…“ (`move_targets`) und jede Verschiebung kommen aus Rust.

- **Lesen** (`CoreHolder`): der Plan wird nur gelesen, solange der Screen ihn zeigt (`PlanQuery` aus Fenster und
  Filter); der einzige Reader-Coroutine ruft beim Verlassen `leave_plan()`. Der `PlanViewModel` hält Filter,
  ersten sichtbaren Tag und Entwürfe pro Tag im SavedStateHandle (`PlanInputs`); nach einem
  Prozess-Tod öffnet der Plan am zuletzt gezeigten Tag. Ein neues Fenster entsteht nur, wenn `plan_window` mit dem
  gebauten Fenster als `built` ein anderes liefert (Hysterese im Core). Während eines Drags meldet die Liste
  keine sichtbaren Tage, das Fenster bleibt also stehen.
- **Compact** (`ui/plan/PlanList.kt`): eine `LazyColumn` aller Tage mit `stickyHeader` pro Tag, Add-Feld,
  QUEST WORK und (wenn gezeigt) OTHER. Kopf: Monat des ersten sichtbaren Tags, Today, „Go to date“, Quest-Filter
  (Chip hebt ihn auf), OTHER ein/aus. `showOther` und die Add-Position teilt der Plan mit Home.
- **Medium/expanded** (ab 600 dp, `PlanBoard.kt`): Tagesspalten nebeneinander in einer `LazyRow` mit
  Snap-Fling; 2–5 Spalten, mindestens 260 dp breit; jede Spalte scrollt ihre Karten selbst. Today setzt heute in
  die zweite Spalte (wie am Desktop einen Tag davor).
- **Drag** (`PlanDrag.kt`): Long-Press (Haptik `LongPress`) hebt die Karte an; eine eigene Geste konsumiert danach
  im `Initial`-Pass, damit weder Liste noch Swipe die Bewegung nehmen. Die schwebende Karte bewegt sich per
  `graphicsLayer`, die Drop-Linie per `derivedStateOf` je Zeile, ohne die Liste neu zu komponieren. Ziel ist die
  nächste angezeigte Karte des Tages im selben Abschnitt (`plan_move_task(id, day, before)`); Tageswechsel gibt
  `SegmentTick`. In der Randzone scrollt eine `withFrameNanos`-Schleife mit `plan_edge_speed` (Liste vertikal,
  Board auch seitlich). Long-Press ohne Bewegung öffnet „Move to…“ (Ziele aus Rust, dazu „Pick a date…“).
- **Accessibility:** jede Karte hat Custom Actions Move to…, Move up/down (`plan_nudge_task`), Move to
  previous/next day (`plan_shift_task`), zusätzlich zu Complete/Delete.
- **TaskCard enger:** Steuerelemente 36 dp (`QuestSpacing.cardControl`), Touch-Ziele bleiben 48 dp
  (`minimumInteractiveComponentSize`); Titel ohne 48-dp-Mindesthöhe.
- **Tests:** `PlanRowsTest` (Zeilen, Drop-Ziel, No-op), `PlanViewModelTest` (Fenster, Hysterese, Prozess-Tod,
  Filter, Hinzufügen, Moves, Fehler), `PlanScreenTest` (Accessibility-Actions, Add-Feld), `CoreHolderTest`
  (Plan nur auf Anfrage, `leave_plan`). Roborazzi hell/dunkel × Phone/Tablet: Plan, Plan mit Filter und Toast,
  Drop-Linie, „Move to…“.
- **Perf:** `docs/perf/plan-android.md` (Emulator, Debug: Scrollen 4,6 % janky, Board 6,5 %, Drag 10 %).

## Phase 4 (Android): Calendar, Log work, Zeiteinträge

Kotlin liest `calendar(query, now)`; Tage, Labels, Summen, Scroll-Ziel, Nachbarseiten, Snapping, Prüfungen und
Speichern kommen aus Rust. Kotlin rechnet nur Picker-Werte in Minuten um (`hour*60+minute`).

- **Lesen** (`CoreHolder.setCalendarQuery`): die Seite wird nur gelesen, solange der Calendar sichtbar ist
  (`CalendarQuery(firstDay, dayCount)`). `CalendarViewModel` hält ersten Tag, offenes Sheet mit Formular,
  Task-Suche und Speicherfehler im SavedStateHandle (`CalendarInputs`); Prüfungen (`check_log_work`,
  `check_entry`, `task_choices`) laufen bei jeder Änderung per `core.read`. Ein anderswo gelöschter Eintrag
  (`UnknownEntry`) schließt den Editor (Collector wie beim Task-Sheet).
- **Layout** (`ui/calendar/`): Tage nebeneinander nach Breite (< 600 dp: 1, < 840 dp: 3, sonst 7). Kompakt:
  Monatstitel, Pfeile, Today (wenn heute nicht sichtbar), darunter Wochenlabel und Wochenstunden; FAB
  „Log work“. Breit: Wochenlabel, Pfeile, Today, „Log work“-Button, Wochenstunden. Seitenwechsel auch per
  Wischen (25 % Schwelle). Der Raster scrollt pro Besuch einmal auf `scroll_minute`.
- **Gesten** (`CalendarGrid.kt`): eine eigene `awaitEachGesture`-Schleife. Griffe (Pillen an den Kanten,
  Zone 48 × 32 dp) ziehen nach dem vertikalen Slop sofort; sonst Long-Press (Haptik) → MOVE auf beweglichen
  Blöcken, NEW_SPAN auf leerer Fläche. Vor dem Long-Press wird nichts konsumiert, Scrollen und Wischen bleiben
  frei. Jede Bewegung fragt `grid_drag_preview` (Geist), Loslassen ruft `grid_drop`: Move/Resize speichert mit
  Undo-Toast („Moved/Changed …“), eine neue Spanne öffnet Log work mit ihrem Entwurf. Tageswechsel gibt
  `SegmentTick`. Tipp: Sitzung → Entry-Editor, geplanter Block → Task-Sheet.
- **Sheets** (`CalendarSheets.kt`, `TaskPickerPage.kt`, `FormFields.kt`): Log work (Task, Datum, Start/Ende,
  Dauer, Quest-Auswahl wenn `can_choose_quest`) und Entry-Editor (Task + leeren, Open task, Start, Ende oder
  „running“, „Ends on a different day“, Delete). Fehler stehen am Feld; ein fehlender Task zeigt sich nur als
  deaktivierter Button. Task-Suche als zweite Seite im Sheet, mit „Create “…”“ in Log work.
- **P3-Nacharbeit:** Karten ohne Quest zeigen ein Assign-Icon („Assign quest“, Menü der Quests) statt des
  „No quest“-Chips; die Steuerzeile bricht um (`FlowRow`), statt den Quest-Chip zu kürzen.
- **Accessibility:** Blöcke tragen `a11y_label` aus Rust und eine Klick-Aktion („Open task“ / „Edit time entry“);
  Formularfelder sind Buttons mit „Label: Wert“ und Fehler-Semantik.
- **Tests:** `CalendarViewModelTest` (Abfrage nur sichtbar, Seiten, Prozess-Tod, Preview/Drop, Log work mit
  neuer und bestehender Aufgabe, Speicherfehler, Editor, gelöschter Eintrag), `CalendarInteractionTest` (FAB,
  Tipp und a11y-Klick, Long-Press-Drag, Griff, neue Spanne, abgebrochene Geste). Ein fehlgeschlagener Check
  bleibt im Sheet (`sheetError`), nicht in der Meldung; nur `UnknownEntry` schließt den Editor. Roborazzi hell/dunkel × Phone/Tablet: Calendar 1/3/7 Tage, Toast,
  Drag-Geist, Log work (leer, neue Aufgabe mit Fehler, Suche, Create), Entry-Editor (normal, über Mitternacht).

## Phase 5 (Android): Quests, Quest-Seite, Review, Archiv, Suche

Kotlin zeigt, was `quests`, `quest_detail`, `review`, `archive` und `search` liefern; Zahlen, Labels,
Trend- und Balkenanteile, a11y-Texte und Prüfungen kommen aus Rust.

- **Lesen** (`CoreHolder`): `setQuestsShown`, `setQuestPage(questId)`, `setReviewQuery(week)`,
  `setArchiveShown` – gelesen wird nur, was sichtbar ist. `UnknownQuest` beim Lesen der Quest-Seite wird
  zu `QuestPage.Gone`; der ViewModel schließt die Seite dann ohne zu speichern. Die Suche liest per
  `core.read` nur, solange sie offen ist (sonst liefe sie bei jedem Tick mit).
- **Eingaben** im SavedStateHandle: Tab und „New quest“-Titel (`QuestsInputs`), offene Quest mit Titel,
  Beschreibung, Task-/Milestone-Entwurf und Meilenstein-Umbenennung (`QuestDetailInputs`), Review-Woche, Suchtext und
  -modus (`SearchInputs`, `SearchMode` als `SavedSearchMode`). Das Suchfeld ist bewusst nicht saveable: ein
  gespeichertes Feld brachte alte Eingaben zurück bzw. überschrieb nach Process Death die gespeicherte.
- **Navigation:** Quest-Seite ist ein Vollbild über allem außer Focus (Predictive Back, Task-Sheet darüber);
  Suche ist ein eigenes Vollbild im Detail-Stapel; Archiv ist ein Detail (`Detail.Archive`). `SearchOutcome`
  wird in `ui/AppDestinations.kt` ausgeführt (Tab, Quest-Seite, Focus, Theme, Account, Log work im Calendar);
  `Choose` bleibt im ViewModel (Liste wechselt, Feld leer).
- **Layout:** Quests als `LazyVerticalGrid` (Adaptive 300 dp), FAB kompakt/Button breit. Quest-Seite ab
  840 dp Tasks und Milestones nebeneinander. Review einspaltig, ab 840 dp Kennzahlen in einer Reihe und Charts
  zweispaltig; Charts per Canvas.
- **Bottom-Sheets:** App-weit nur voll offen (`rememberBottomSheetState(Hidden, {Hidden, Expanded})` in `QuestBottomSheet`,
  Task-Sheet und Calendar-Sheets; `rememberModalBottomSheetState(skipPartiallyExpanded)` ist deprecated): halb offen verdeckte die Tastatur Feld und Buttons.
- **Tests:** ViewModel-Tests für Quests, Quest-Seite, Review, Archiv, Suche; `AppNavigationTest` (Suche →
  alle Outcomes, Suche wieder leer, Restore aus dem Archiv, Milestone hinzufügen), `SearchScreenTest`
  (Process Death), `LogWorkQuestTest`. Roborazzi hell/dunkel × Phone/Tablet für alle neuen Screens.

## Phase 6 (Android): Widget, Hintergrund-Sync, Quick-Add, Shortcuts, Kachel

Ein Core pro Prozess (`QuestApplication.core`) für App, Widget, Worker, Quick-Add und Kachel; ein kalter
Prozess (Worker, Widget, Teilen) wartet mit `CoreHolder.awaitOpen()`. Kotlin rechnet weder Zeiten noch Labels.

- **Flush:** `ProcessLifecycleOwner.onStop` → `saveForBackground` (`sync/BackgroundSave.kt`): `core.flush(2000)`
  auf IO, danach bei Pending > 0 (`Connecting` zählt mit) ein One-Shot-Worker `quest-sync-now`. Auch der
  Widget-Haken ruft es auf. Nachweis: Aktion offline, Home, sofort Kill, Neustart → Änderung da, „1 pending“.
- **Worker** (`sync/SyncWorker.kt`, `SyncScheduler.kt`): CoroutineWorker, `syncUntilIdle(60 s)` →
  `Idle` success, `Offline`/`TimedOut` retry (exponentiell ab 30 s), Core nicht offen failure, abgemeldet
  success. Danach Widget-Update. Periodisch `quest-sync-periodic` (15 min, `CONNECTED`, `KEEP`: ein
  laufender Zeitplan bleibt beim App-Start erhalten), One-Shot `quest-sync-now` (`CONNECTED`, `KEEP`: ein
  wartender Lauf schickt ohnehin alles). Im Vordergrund ruft der Worker `syncUntilIdle` trotzdem auf; der
  Core synct dann schon live und kehrt sofort zurück. Entscheidung `syncRunResult` ist pur und getestet.
- **Widget** (Glance 1.2, `widget/`): `SizeMode.Exact`; unter 250×110 dp nur Stunden (heute/Woche), sonst
  Tageslabel, Stunden und so viele Aufgaben (Quest Work zuerst), wie in die Höhe passen, plus „+N more“.
  Haken (48 dp) → `ToggleTaskAction` → `toggleTask`; Titel → `quest://task/<id>` (Task-Sheet); sonst → App.
  Farben aus den Quest-Tokens (`ColorProvider(day, night)`, folgt dem System). `WidgetUpdater` liest `home`
  nur, wenn Widgets existieren: bei `CoreHolder.changes` (conflated, höchstens alle 2 s; Ticks feuern nicht),
  bei Tageswechsel im Prozess und per Alarm um `next_day_ms` sowie bei Uhrzeit-/Zeitzonenwechsel (nicht exportierter
  `DayChangeReceiver`), nach jedem Worker-Lauf. Glance kappt Container bei 10 Kindern: höchstens 10 Zeilen. Zeilenzahl aus der
  `LocalSize`-Höhe abzüglich des gemessenen Kopfs (60 dp inkl. 8 dp vertikalem Padding).
- **Quick-Add** (`quickadd/`): transparente `QuickAddActivity` (eigene Task-Affinity) mit Bottom-Sheet; Ziel
  für `ACTION_SEND text/plain` (Subject und Text, durch Zeilenumbruch verbunden), den Shortcut „New task“ und
  die Kachel. `quick_add_form` liefert Titel/Beschreibung aus dem geteilten Text, Quest-Vorschlag und heute;
  `picked_day` labelt einen anderen Tag; `quick_add` prüft. Eingaben im SavedStateHandle (`QuickAddInputs`).
- **Shortcuts:** statisch „New task“ (Quick-Add) und „Log work“ (Calendar + Log-work-Sheet), dynamisch
  „Start timer“ (Langlabel mit dem Task aus `focus`, nach Core-Start und bei jedem `onStop` neu). „Start
  timer“ startet die letzte Aufgabe über `FocusViewModel.startLastTask` und öffnet Focus.
- **Kachel:** `NewTaskTileService` öffnet Quick-Add (ab API 34 `startActivityAndCollapse(PendingIntent)`,
  gesperrt erst nach `unlockAndRun`).
- **Deep Links/Requests:** `MainActivity` übersetzt Intents in `AppRequest` (`OpenTask`, `LogWork`), die
  `QuestApp` nach dem Öffnen des Cores ausführt.
- **P5-Nacharbeit:** Quest-Seite auf Tablets auf 840 dp zentriert; Task-Sheet und New-Quest-Sheet in
  Roborazzi in M3-Sheet-Breite (640 dp) statt randlos.
- **Tests:** `SyncPolicyTest`, `SyncWorkerTest` (`TestListenableWorkerBuilder`, Kalt-Start, Offline,
  TimedOut, abgemeldet, Core kaputt), `CoreHolderTest` (`changes`, `flush`/`syncUntilIdle`),
  `QuickAddViewModelTest`, `SharedTextTest`, `WidgetContentTest` (`glance-appwidget-testing`),
  `WidgetScreenshotTest` (Glance → RemoteViews → Roborazzi, klein/mittel/groß × hell/dunkel), Roborazzi
  Quick-Add hell/dunkel × Phone/Tablet.

## Phase 7 (Rust): Größe der `.so`, TLS-Backend, x86_64

- **Profil `mobile-release`** (Root-`Cargo.toml`, von `apps/android/build-rust.sh` genutzt): erbt `release`,
  `lto = "fat"`, `codegen-units = 1`, `opt-level = "s"`. `panic` bleibt `unwind`: `guard` (`catch_unwind` an jeder
  exportierten Methode) macht aus einem Panic `MobileError.Internal`; mit `abort` würde derselbe Panic die App
  beenden. Das kostet ~0,6 MB (Tabelle).
- **`"s"` statt `"z"`:** `"z"` spart installiert 0,3 MB (5 %), im Download (gzip) nichts; dafür gibt `"z"`
  Laufzeit auf (keine Schleifen-Vektorisierung, mehr Aufrufe statt Inlining). Bei gleichem Download überwiegt das.
- **`ring` statt aws-lc:** reicht. Quest braucht nur einen TLS-1.2/1.3-Client mit Standard-Cipher-Suites; FIPS
  oder Post-Quantum-Kex (die Vorteile von aws-lc) braucht es nicht. `reqwest` mit `rustls-no-provider` bekommt
  die fertige Konfiguration per `tls_backend_preconfigured`, damit nur ein Provider im Binary landet. aws-lc ist
  damit aus dem ganzen Workspace (auch Desktop) verschwunden; ring baut ohne CMake/NASM und schneller.
- **ABIs:** `arm64-v8a` und `x86_64` (`QUEST_ANDROID_ABIS`, Default beide). Die Bindings kommen immer aus der
  arm64-Bibliothek.
- **Warnung „comparison is useless due to type limits“:** stammte aus `quest-store/src/idle.rs`
  (`*attempt >= 0`, ein Zwischenstand der Retry-Prüfung, im Cargo-Cache noch sichtbar). Der aktuelle Code
  (`>= 2`) baut für Android warnungsfrei; die Build-Script-Warnungen von aws-lc-sys sind mit aws-lc weg.

Größe von `libquest_mobile.so` (gestrippt wie im APK mit `llvm-strip --strip-unneeded`; gzip -9 als Näherung für
den Download; NDK 30, Rust 1.99):

| Variante | arm64-v8a | gzip | x86_64 |
|---|---:|---:|---:|
| vorher: `release`, aws-lc | 9,68 MB | 4,10 MB | – |
| `release`, ring | 8,23 MB | 3,42 MB | – |
| LTO + cgu 1, `"s"`, aws-lc | 6,97 MB | 2,80 MB | – |
| LTO + cgu 1, `"z"`, aws-lc | 6,64 MB | 2,80 MB | – |
| LTO + cgu 1, `"s"`, aws-lc, `panic=abort` | 6,36 MB | 2,55 MB | – |
| LTO + cgu 1, `"z"`, ring | 6,12 MB | 2,62 MB | – |
| LTO + cgu 1, `"s"`, ring, `panic=abort` | 5,84 MB | 2,38 MB | – |
| **gewählt: `mobile-release` (LTO + cgu 1, `"s"`, ring, unwind)** | **6,43 MB** | **2,62 MB** | **6,71 MB** |

Ergebnis: −34 % installiert, −36 % Download pro ABI. Messskript: `/tmp/p7-size/bin/measure.sh` (nicht im Repo).

## Phase 7 (Android): Release, Performance, Barrierefreiheit

- **Release-Build:** R8 mit `isMinifyEnabled` und `isShrinkResources`; Keep-Regeln (`app/proguard-rules.pro`)
  nur für das, was per Name erreicht wird: JNA (komplett), `uniffi.**` (JNA bindet `external fun`s und liest
  Struct-Felder per Reflection), `org.rustls.platformverifier.**` (JNI aus Rust) und Glance-`ActionCallback`s
  (Glance legt den Klassennamen in den PendingIntent und baut die Klasse per No-arg-Konstruktor; ohne Regel tat
  der Widget-Haken in Release nichts – `NoSuchMethodException`, im Emulator gefunden). WorkManager, Compose und
  AndroidX bringen eigene Consumer-Regeln mit.
- **Signing:** `QUEST_RELEASE_STORE_FILE`, `…_STORE_PASSWORD`, `…_KEY_ALIAS`, `…_KEY_PASSWORD` aus Gradle-Properties
  oder Umgebung. Ohne Store-Datei bleibt Release unsigniert (lokal baubar); mit Store-Datei sind alle vier Pflicht,
  sonst bricht der Build mit Namen der fehlenden Werte ab. Kein Schlüssel im Repo (`apps/android/.gitignore`:
  `*.jks`, `*.keystore`, `keystore.properties`); zum Testen `/tmp/quest-release-test.jks`.
- **Version aus einer Quelle:** `apps/android/version.properties` (`versionName`); `versionCode =
  major·1 000 000 + minor·1 000 + patch` (geprüft: Format `x.y.z`, minor/patch < 1000). Der Release-Workflow
  verlangt Tag `android-v<versionName>`.
- **ABIs:** Gradle-Property `quest.abis` (Default `arm64-v8a x86_64`) → `abiFilters` und Env `QUEST_ANDROID_ABIS`
  für `build-rust.sh`; Input des `cargoBuild`-Tasks, der vor dem Bauen `jniLibs` leert (kein Rest einer
  abgewählten ABI). Debug baut dieselben ABIs: ein Debug-APK läuft überall, wo das Release läuft, und der zweite
  ABI kostet inkrementell kaum Zeit (Cargo-Cache); lokal schneller mit `-Pquest.abis=arm64-v8a`.
- **Baseline Profile:** Modul `:baselineprofile` (`com.android.test` + `androidx.baselineprofile` 1.5.0,
  uiautomator) mit `BaselineProfileGenerator` (Start, Home, Plan, Calendar) und `StartupBenchmark`
  (ohne Kompilierung vs. Profil). Profil eingecheckt, profileinstaller 1.4.1 in der App. Kaltstart Release
  194 → 168 ms (Debug vorher 762 ms), Plan-Jank p99 48 → 21 ms: `docs/perf/android-startup.md`.
- **Barrierefreiheit:**
  - `heading()` an Screen-Titeln, Abschnitten (`SectionHeader`, `Caps`), Sheet-Titeln, Datum (Home), Monat (Plan),
    Focus-Auswahl. Test: jede Hauptseite hat eine Überschrift (`AppNavigationTest`).
  - Calendar-Blöcke: `customActions` „Move 15 minutes earlier/later“ und „End 15 minutes earlier/later“ über
    denselben `grid_drop` wie der Drag (15 min = Raster des Grids). Test: `CalendarInteractionTest`. Plan- und
    Home-Karten hatten ihre Aktionen schon (Phase 1/3).
  - Schrift 200 %: Roborazzi `PhoneLargeFontScreenshotTest`/`TabletLargeFontScreenshotTest` (19 Szenen, hell,
    `fontScale = 2f`). Behoben: Nav-Labels (Umbruch mitten im Wort → `CappedFontScale(1.3)`, eine Zeile),
    Stundenspalte im Calendar (`CappedFontScale(1.2)`), Buttonzeilen und Kennzahlen als `FlowRow`, Plan- und
    Calendar-Kopf brechen um statt abzuschneiden, Home-Stundenzeile. Bewusst gedeckelt nur Nav-Labels und
    Stundenspalte (feste Breiten/Höhen); kurze Calendar-Blöcke schneiden ihren Titel weiter ab (Höhe = Dauer).
  - Kontraste: Androids Accessibility Test Framework (`enableAccessibilityChecks`) läuft unter Robolectric,
    prüft dort aber nichts (Probe mit Kontrast 1,4 : 1 und 10-dp-Ziel bestand) – kein Ersatz, wieder entfernt.
    Stattdessen `QuestColorsContrastTest` (WCAG-Verhältnis aus den Tokens): `text`/`textMuted` auf allen Flächen
    ≥ 4,5 : 1 in hell und dunkel. Darunter liegen (gemessen, nicht geändert, weil Desktop-Tokens aus
    `theme.json`; Frage an den Lead): hell `accent` als Text auf `bg` 2,6 : 1 (Text-Buttons, „heute“, Fehlertext
    in Formularen), Weiß auf `accent` 2,8 : 1 (FABs, gefüllte Buttons), `failed` auf `bg` 3,3 : 1, `textMuted` auf
    `selected` 4,4 : 1; `textFaint` 2,4 : 1 (erledigte Aufgaben und Platzhalter, nach WCAG ausgenommen). Dunkel
    liegt alles außer `textFaint` über 4,5 : 1.
  - TalkBack: läuft im Emulator, aber per `adb shell input` gespritzte Gesten umgehen die Touch-Exploration
    (ein Wisch löste die Swipe-Geste der Karte aus statt „nächstes Element“). Beleg daher über den
    Accessibility-Baum (`uiautomator dump`, was TalkBack vorliest, in Reihenfolge) plus Compose-Semantik-Tests.
- **Phase-0-Rest:** Splash folgt der App-Darstellung: `UiModeManager.setApplicationNightMode` (ab API 31) bei
  jeder Änderung der Einstellung, `windowSplashScreenBackground` = `bg` hell/dunkel (vorher Weiß). Passwort-Autofill:
  E-Mail `ContentType.EmailAddress + Username`, Passwort `OutlinedSecureTextField` mit `Password + NewPassword`
  und `isSensitiveData`. Gradle-Dependency-Verification: `apps/android/gradle/verification-metadata.xml`
  (SHA-256 für alle aufgelösten Artefakte, auch `org.rustls:rustls-platform-verifier` AAR/POM aus dem
  GitHub-Maven-Archiv); erzeugt mit den Gate-Tasks plus `assembleRelease` und dem Baseline-Profile-Modul.
- **P6-Nacharbeit:**
  - Widget: offene Aufgaben vor erledigten, darin Quest Work zuerst (`widgetTasks`).
  - Widget-Stunden: der Updater reagiert auf geänderte Stunden-Texte (`ShownTimes`), nicht nur auf den Tag.
    Der Core tickt, solange die App sichtbar ist oder der FocusService läuft; der Text ändert sich höchstens alle
    6 Minuten (`format::hours`, Zehntelstunden). Kein eigener Minuten-Alarm (Akku, Glance-Kosten).
  - `quest://task` und „Start timer“ nur noch über den nicht exportierten `activity-alias` `.InternalEntry`
    (Widget, Shortcut). Von außen: `quest://task` implizit nicht auflösbar, `.InternalEntry` → SecurityException,
    `START_TIMER` an `MainActivity` wird ignoriert. Wer `MainActivity` explizit mit `quest://task/<id>` startet,
    öffnet nur das Sheet (keine Änderung).
  - `syncUntilIdle` abbrechbar: `CoreHolder` wartet in 1-s-Scheiben (`SYNC_SLICE_MS`) und prüft dazwischen auf
    Abbruch; ein gestoppter Worker endet nach höchstens einer Scheibe.
  - **Worker-Fehler in Release gefunden:** zwischen zwei Jobs sperrt Android einem gecachten Prozess das Netz
    (`blocked=APP_BACKGROUND`). Der weiterlebende Core sammelt fehlgeschlagene Retries; startete der nächste Job,
    meldete `syncUntilIdle` sofort `Offline` → `RETRY` nach 6 ms, immer wieder. Jetzt fragt `CoreHolder` einen
    offline Core (nicht abgemeldet/fehlgeschlagen) bis zum Zeitbudget jede Sekunde neu; der nächste Core-Retry
    (≤ 30 s Backoff) läuft dann mit dem Netz des Jobs. Nachweis: `SUCCESS` nach 17 s, Peer sieht die Änderung.
    Sauberer wäre ein `reconnect_now` im Core (Frage an den Lead).
- **Release-Workflow:** `.github/workflows/android-release.yml` (Tag `android-v*`; Actions auf Commit-SHAs
  gepinnt; Tag gegen `version.properties`; Gradle-Wrapper-Validierung; mise, Rust-Targets, NDK). Gradle baut
  das APK **unsigniert** (kein Secret im Gradle-Lauf); erst `apps/android/sign-release.sh` bekommt die Secrets,
  dekodiert den Keystore in ein Temp-Verzeichnis (per `trap` gelöscht), `zipalign -P 16 -f 4` (16-KB-Seiten für die
  unkomprimierte `.so`), `apksigner sign` mit `env:`-Passwörtern, ohne v4-`.idsig`, und vergleicht das
  Zertifikat-SHA-256 mit der Variable `QUEST_RELEASE_CERT_SHA256` (fehlt oder abweichend → Abbruch). Build-Job
  nur lesend und ohne gespeicherte Git-Credentials, Schreibrecht nur im Release-Job (Upload per Artifact, Re-Run
  ersetzt die Dateien). Lokal bleibt das Signieren über Gradle (`~/.gradle/gradle.properties`); `QUEST_RELEASE_*`
  in der Repo-`gradle.properties` bricht den Build ab, der Cargo-Build bekommt die Werte nicht. `actionlint` und
  `shellcheck` ohne Befund; Skript lokal mit dem Test-Key geprüft (richtiger Fingerprint → signiert, falscher →
  Abbruch ohne APK). Installation und Updates per Obtainium: `apps/android/README.md`.
- **Klartext nur im Debug-Build zu 10.0.2.2:** `RustQuestCore` gibt `UrlPolicy.DEVELOPMENT` nur bei
  `BuildConfig.DEBUG` an `signIn`/`checkServerUrl`, sonst `RELEASE` (http nur zu Loopback). Die Server-URL startet
  im Release leer mit Platzhalter `https://quest.example.com` (es gibt keinen öffentlichen Server, den man
  vorschlagen könnte, und 10.0.2.2 würde abgelehnt). Network-Security-Config: `src/main` ohne Klartext-Ausnahme,
  `src/debug` mit 10.0.2.2. Release gegen den lokalen Server: `adb reverse tcp:8787 tcp:8787` und
  `http://127.0.0.1:8787`.
- **Sperrbildschirm:** Die Focus-Notification ist `VISIBILITY_PRIVATE` mit `setPublicVersion`: „Focus“ bzw.
  „Focus paused“ mit Chronometer, ohne Aufgabe, Quest und Buttons. Der Platzhalter beim Service-Start (nur
  „Focus“) ist ausdrücklich `VISIBILITY_PUBLIC`. Tests: `FocusNotificationsTest`.
- **Logs:** Fehlgeschlagene Notification-Aktionen loggen nur die Art (`Failed`, `NotReady`), nicht die
  Core-Meldung (sie kann Aufgaben oder den Server nennen). Die FGS-Warnungen loggen weiter die System-Exception
  (enthält nur Paket und Service).
- **Bewusst so (Review):** Das Session-Token liegt unverschlüsselt in der SQLite-Datei des Cores in der
  App-Sandbox, ohne Backup und Gerätetransfer (`allowBackup=false`, `data_extraction_rules`); Restrisiko nur mit
  Root/forensischem Zugriff. Eine Verschlüsselung mit einem Android-Keystore-Schlüssel wäre eine eigene
  Produktentscheidung. Panics des Cores zeigen ihr Detail weiter in der Meldung
  (`<Text> (InternalException: …)`), damit Nutzer es in Fehlerberichten mitschicken können.
- **Release-Prüfung im Emulator** (signiert mit Test-Key): Start, Anmeldung, Sync mit Peer in
  beide Richtungen (ring), HTTPS-Ablehnung eines selbstsignierten Servers (Verifier über JNI überlebt R8),
  FocusService mit Notification, Shortcut „Start timer“, Widget (Reihenfolge, Haken → Peer, Titel → Sheet),
  Worker, Splash hell/dunkel.

## Phase 8 (Rust): AI-Erfassung

- **Ablauf:** Client → `quest-store` (`Store::ai().capture`, blockierend, Session-Token, keine Redirects) →
  `quest-server` `POST /v1/ai/capture` → OpenAI-kompatibler Provider (Structured Outputs, strikt; Fallback
  Tool-Calling) → `quest_capture::validate` → `CaptureProposal`. Schlüssel und Provider-Aufruf liegen nur im
  Server (`crates/quest-server/README.md`); Diktattext, Prompt und Antwort werden nirgends geloggt.
- **`quest-capture`** (pur, geteilt von Server, Desktop und Mobile): Kontext (aktive Quests, Tasks −7…+14 Tage,
  höchstens 150), Prompt und Schema, Validierung der nicht vertrauenswürdigen Antwort (temporäre Schlüssel
  `q1`/`t1`/`w1`, Datumsgrenzen, Blöcke bis Mitternacht, Sessions nicht in der Zukunft, „Left out“-Liste),
  `review::Draft` (Auswahl, Fehler, Labels) und `plan`/`undo` (Commands bzw. Rücknahme). Keine Schätzungen:
  eine Dauer ohne Startzeit wird ignoriert.
- **Alles oder nichts:** `Store::dispatch_all` plant jedes Command gegen den Stand nach den vorigen und
  übernimmt nur, wenn alle gelingen.
- **UniFFI:** `capture(text, language, now)` (Netz, nicht auf dem Main-Thread; die Store-Sperre ist dabei nicht
  gehalten), `capture_view(draft, now)` (Labels, Fehler, Quest-Optionen, „Create (n)“), `apply_capture(draft,
  now)` und `reconnect_now()` (überspringt das Backoff, z. B. vor `sync_until_idle` im Worker). Fehler:
  `AiSignedOut`, `AiNotConfigured`, `ServerUnreachable`, `AiUnavailable`, `AiRateLimited`, `AiInvalidResponse`,
  `CaptureTextTooLong`.
- **Zeiten:** Picker liefern Minuten; ein Ende 0 ist Mitternacht am Ende des Tages, 00:00 bis Mitternacht ist
  keine Session. Ortszeit → UTC wie überall: in der doppelten Stunde der frühere Zeitpunkt, eine übersprungene
  Zeit eine Stunde später.

## Phase 8 (Android): AI-Erfassung

Eine transparente `CaptureActivity` (eigene Task-Affinity, wie Quick-Add) über allem, was gerade offen ist:
erst Diktat-/Text-Sheet, dann Vorschlag. Sie teilt den Core des Prozesses. Kotlin rechnet weder Zeiten noch
prüft es: Es setzt nur Draft-Felder mit Picker-Werten und liest danach `capture_view` neu.

- **Einstiege:** Mikrofon im Titelfeld von Quick-Add (übergibt den getippten Titel), Share-Ziel „Structure with
  AI“ (exportierter `activity-alias .capture.StructureWithAi`, `SEND text/plain`; über den Alias zählt nur ein
  `SEND`, eigene Aktionen und Extras werden ignoriert, auch wenn eine App ihn direkt benennt),
  dynamischer Shortcut „Dictate“ (`ACTION_DICTATE`, hört sofort zu, wenn die Berechtigung schon da ist) und
  „Capture with AI“ über den Befehlen der Suche (übergibt die Anfrage). Statt einer zweiten QS-Kachel ein
  Shortcut: ein Tile-Service lässt sich nicht pro Einstellung ausblenden, ohne die Kachel zu entfernen.
- **Ein/aus:** `CapturePreferences` (DataStore `capture.enabled`, Standard an; `capture.privacy_acknowledged`).
  `entriesShown` = an und nicht `AiNotConfigured`; `QuestApplication` schaltet daraus Alias
  (`setComponentEnabledSetting`) und Shortcut (ein angehefteter wird deaktiviert), App, Suche und Quick-Add
  blenden ihre Knöpfe aus.
  `AiNotConfigured` merkt sich die App nur im Speicher bis zum nächsten Start: die Server-Konfiguration kann
  sich jederzeit ändern, und ein dauerhaftes Aus fände niemand wieder.
- **Diktat** (`capture/Dictation.kt`): `SpeechRecognizer` lebt in der UI (Main-Thread, `DisposableEffect`,
  `destroy` beim Verlassen, `ON_STOP` bricht ab). On-device, wenn `isOnDeviceRecognitionAvailable`, bei
  `LANGUAGE_NOT_SUPPORTED/UNAVAILABLE` Rückfall auf den System-Recognizer; Sprache
  `Locale.getDefault().toLanguageTag()`, `EXTRA_PARTIAL_RESULTS`. Teiltext erscheint live („Heard“, Live-Region),
  das Endergebnis wird in einer neuen Zeile an den editierbaren Text gehängt. `RECORD_AUDIO` erst beim ersten
  Tippen, vorher eine eigene Erklärung; bei Ablehnung bleibt die Texteingabe mit Hinweis. `<queries>` für
  `RecognitionService` (Paketsichtbarkeit).
- **Strukturieren:** Erstes Senden zeigt einmal den Datenschutz-Hinweis (Text geht über den Quest-Server zum
  Anbieter, mit Quest- und Task-Namen). `capture` läuft über `CoreHolder.readOnIo` (wartet auf das Öffnen,
  dann IO); Cancel bzw. Zurück bricht ab und lässt den Text stehen. Fehler (`ServerUnreachable`, `AiUnavailable`,
  `AiRateLimited`, `AiInvalidResponse`, `CaptureTextTooLong`, …) bleiben im Text-Sheet; `MobileException` ist
  flach, daher zeigen Offline und zu lang die Meldung des Cores.
- **Vorschlag** (`capture/ProposalSheet.kt`): Gruppen New quests / Tasks / Time entries mit Checkbox und
  aufklappbarer Bearbeitung (Titel, Quest-Chips aus `quest_options`, Tag über den vorhandenen Day-Picker oder
  Someday, Block-Start/-Ende über `TimeField`, Repeat über den `RepeatEditor` des Task-Sheets). Unsichere
  Zuordnung mit Icon und Text, `dropped` als Hinweis „Left out“, Fehler pro Eintrag aus dem View, Knopf
  `apply_label` („Create (n)“), aktiv bei `can_apply`. Blöcke und Wiederholungen lassen sich ändern oder
  entfernen, nicht neu anlegen. Zurück (auch Predictive Back) schließt erst einen offenen Repeat-Editor, dann
  geht es zum Text und verwirft den Entwurf. Create → `apply_capture` (als Aktion, markiert die Änderung), die
  Meldung des Cores als Snackbar, danach schließt die Activity.
- **Prozess-Tod:** Text, Entwurf, aufgeklappter Eintrag und Repeat-Formular liegen im SavedStateHandle
  (`CaptureInputs`, `Serializable`-Spiegel der UniFFI-Records in `capture/CaptureModel.kt`, Muster wie
  `QuickAddInputs`; kein Parcelize-Plugin nötig, der Entwurf ist klein). Text höchstens 50 000 Zeichen, damit ein
  geteilter Artikel das Binder-Limit nicht sprengt (der Core lehnt weit früher ab). Fortschritt, Fehler und
  Ergebnis sind flüchtig: Ein Strukturieren läuft nach dem Tod nicht weiter, der Text bleibt. Der Vorschlag wird
  neu gelesen, sobald der Core offen ist und wenn sich seine Daten ändern (`CoreHolder.state`/`changes`).
- **Sync:** `SyncWorker` ruft `reconnect_now()` vor `syncUntilIdle`, damit ein Core im Backoff sofort neu
  verbindet.
- **P7-Nacharbeit:** Segment „Active | Completed“ mit Zahl als Badge, einzeilig mit Ellipse (200 %);
  kurze Calendar-Blöcke: Titel einzeilig, Zeit nur, wenn sie passt; Widget ohne Punkt bei Aufgaben ohne Quest;
  `distributionSha256Sum` im Gradle-Wrapper.
- **Tests:** `CaptureStartTextTest` (Share, Alias), `CaptureViewModelTest` (Text, Diktat-Ereignisse, Datenschutz, Senden mit Sprache, Fehler,
  `NotConfigured`, Abbrechen, Auswahl, Bearbeiten, Tag, Block, Repeat, Create, SavedState), `SettingsViewModelTest`
  (Schalter), `SyncWorkerTest` (`reconnect_now`). Roborazzi: Diktat, Zuhören, Strukturieren, Offline,
  Vorschlag, aufgeklappter Task und Zeiteintrag, Quick-Add mit Mikrofon, Suche mit „Capture with AI“ –
  hell/dunkel × Phone/Tablet, die Sheets zusätzlich mit Schrift 200 %.
- **R8:** keine neue Regel; `-keep class uniffi.** { *; }` deckt die neuen Records und Fehler ab.
- **Manuell geprüft:** Suche → Sheet → Berechtigung abgelehnt →
  Datenschutz → Vorschlag → bearbeitet → Prozess-Tod → Create → Sync mit dem Desktop-Peer, Share-Ziel,
  Schalter. Gegen einen eigenen Server mit `ai_mock`; Live-Diktat ist im Emulator nicht belegt (kein Mikrofon).

## Phase 9 (Android): Undo nach KI-Erfassung, verbundene KI-Clients

- **Undo nach „Create (n)“:** `apply_capture` merkt sich im `QuestCore` (eigenes Feld `capture_undo`, nicht im
  `MobileUndo` der Snackbars) die `created`-Liste des Plans; `undo_capture(now)` nimmt sie einmal heraus und wendet
  `quest_capture::undo(&created, &snapshot, at)` mit `dispatch_all` an (löscht weich, was noch existiert, alles oder
  nichts). Eine neue Erfassung ersetzt die alte; nach 2 Minuten (`UNDO_LIMIT`) und beim Abmelden verfällt es. Warum kein Token in `CaptureApplied`: Domain-Ids blieben in Kotlin
  verborgen, ein gefälschtes oder veraltetes Token kann nichts löschen, und es entspricht dem Desktop
  (`State::capture_undo`). Warum nicht im `MobileUndo`: dessen Toast zeigen Home/Plan/Calendar; die Capture-Activity
  liegt transparent darüber, also gäbe es zwei Snackbars. Nach Prozess-Tod ist das Undo weg (die Snackbar auch).
  `CaptureApplied` trägt `undo_label` („Undo“) und `show_for_ms` (`TOAST_SECONDS`, 6 s wie Desktop).
- **Kotlin:** `CaptureActivity` zeigt nach Create `UndoSnackbar` (Undo, Schließen) so lange, wie der Core sagt
  (`show_for_ms`, verlängert über `calculateRecommendedTimeoutMillis` für Bedienungshilfen); Tippen daneben,
  Schließen oder Ablauf beenden die Activity. Undo → `undoCapture` → schließt; Fehler → Meldung als Snackbar, dann zu.
- **Verbundene KI-Clients:** `quest-store` `Store::connections() -> ConnectionsClient` (`list()`, `revoke(id)`,
  blockierend, Session-Token, keine Redirects; `ConnectionsError` `SignedOut`/`Unauthorized`/`Unreachable`/
  `NotFound`/`Server`). `quest-mobile`: `ai_connections(now) -> Vec<AiConnectionRow { id, name, host, detail }>` und
  `revoke_ai_connection(id)` (Netz, nur auf IO; die Store-Sperre ist dabei nicht gehalten). `detail` rechnet Rust:
  „Connected Nov 3 · Last used 2 days ago“ (lokale Kalendertage: today/yesterday/n days ago bis 6, sonst Datum, Jahr
  nur wenn anders; „Not used yet“), leerer Name → „Unnamed AI client“. Neue Fehler: `NotSignedIn`,
  `SessionRejected`, `UnknownConnection`; nicht erreichbar → `ServerUnreachable` (vorher `AiOffline`, gilt
  jetzt auch für die KI-Erfassung).
- **Settings:** Abschnitt „Connected AI clients“ unter AI capture, nur angemeldet. Jeder Besuch lädt neu
  (`SettingsViewModel.loadConnections` über `CoreHolder.readOnIo`). Zustände Laden, Liste, leer, offline bzw. Fehler
  mit „Try again“; Revoke → Bestätigungsdialog → Zeile verschwindet (auch wenn der Client schon anderswo widerrufen
  war), sonst Fehlertext unter der Liste. Kotlin-Fehlertyp `core/ConnectionsException(offline, gone)`;
  ein Neuladen während eines Widerrufs wartet auf diesen.
- **Tests:** Rust `capture_tests` (Undo einmalig, ohne Capture nichts, zwischendurch Gelöschtes bleibt gelöscht),
  `connections_tests` (Labels, Zeitzone, Fehler), `quest-store/tests/connections.rs` (gegen den In-Process-Server, auch mit echtem OAuth-Grant: gelistet,
  widerrufen, weg);
  `CaptureViewModelTest` (Undo, einmalig, Fehler), `SettingsViewModelTest` (abgemeldet, Laden, leer, offline,
  Revoke mit Dialog, Fehler, schon widerrufen, Neuladen während Revoke, Abmelden). Roborazzi: `capture-applied`, `settings-connections*`
  (Liste, Laden, leer, offline, Widerruf läuft, Fehler) hell/dunkel × Phone/Tablet, Liste/offline/Snackbar auch 200 %.
- **R8:** keine neue Regel (`-keep class uniffi.**`).

## Phase 11: Anmeldung über OIDC, Profile, KI-Opt-in

- **Rust (`quest-mobile`):** `QuestCore` hält `quest_store::Profiles` und den aktiven `Arc<Store>`
  (`Store` ist `Send + Sync`). Anmelden in zwei Schritten (RFC 8252): `begin_sign_in(url, policy)` baut
  `LoginRequest::new(url, AppClient::Android)` (PKCE S256, `state`) und gibt die Authorize-URL zurück;
  `finish_sign_in(redirect)` prüft Antwort und `iss`, tauscht den Code (blockierend, nur IO) und ruft
  `Profiles::sign_in`. Ergebnis `SignInResult::{SignedIn{offer_guest_import, account}, Renewed, NotASignIn}`;
  `NotASignIn` ist die Rückkehr vom Abmelden (Redirect ohne `code`/`error`), ein Redirect ohne laufende Anmeldung
  `SignInExpired`. Eine Antwort mit fremdem `state` (andere App mit demselben Schema) wird abgelehnt, ohne die
  wartende Anmeldung zu verbrauchen (`state` aus der eigenen Authorize-URL). `import_guest_data()`, `sign_out_warning()` (Text bei `pending() > 0`), `sign_out()` (gibt die
  Logout-URL `/auth/logout?return_to=…` zurück), `set_ai_enabled(bool)`. Ein Profilwechsel ersetzt den Store im
  Core (`replace_store`: Ticks, Undo, Plan und Capture-Undo zurückgesetzt, `ChangeListener.on_change`), der alte
  wird in einem eigenen Thread freigegeben (wartet bis 1 s auf andere Aufrufer), nie auf dem UI-Thread.
  Ein Mutex `switching` serialisiert Anmelden, Import und Abmelden. Während des Abmeldens lehnt der Core
  Bearbeitungen mit `SigningOut` ab (`RwLock<bool>`: Bearbeitungen halten den Lesezugriff, das Abmelden wartet
  per Schreibzugriff auf laufende), statt sie mit den Kontodaten still zu löschen. Widget, Worker, Kachel, Share und Fokus-Service
  hängen am Core und folgen damit immer dem aktiven Store.
- **Tokens:** foreign Trait `TokenStore` (`load`/`save`/`delete` je Kontoschlüssel) hinter dem Adapter
  `ForeignVault: TokenVault`. Kotlin `auth/TokenFiles.kt`: eine Datei je Konto in `noBackupFilesDir/tokens`,
  AES-256-GCM mit Schlüssel im Android Keystore (`quest-tokens`), Kontoschlüssel als AAD, `AtomicFile`.
- **Fehler:** `SessionExpired` (ersetzt `SessionRejected`), `AiDisabled`, `SignInCancelled`,
  `SignInFailed{reason}`, `SignInExpired`; `SyncState::SessionExpired` → „Sign in again“.
  `Account { display_name, email, name, server_url, ai_enabled, ai_available, ai_features }`;
  `ai_features = ai_enabled && ai_available` entscheidet über alle KI-Einstiege.
  `AiConnectionRow.scope`: „Read only“ / „Read and write“.
- **Redirect:** Private-Use-Scheme `app.quest.android:/oauth2redirect` (RFC 8252 §7.1) statt App Link: braucht
  keine `assetlinks.json` auf jedem selbst betriebenen Server. Gegen fremde Apps mit demselben Schema zeigt der
  Server für `quest-android` immer „Continue as …“, PKCE bindet den Code an die App. `AuthRedirectActivity`
  (exportiert, `noHistory`, durchsichtig) reicht die URL an `AuthRedirects` (Channel) und holt `MainActivity` nach
  vorn. Geöffnet wird mit Custom Tabs (`androidx.browser`).
- **Android:** `AccountViewModel` (Gast mit Anmeldeformular, Warten auf Browser, Import-Dialog, Abmelde-Dialog mit
  Warnung, „Sign in again“); das Import-Angebot überlebt Prozess-Tod (`SavedStateHandle`). Settings → „AI features“
  (Konto-Einstellung, aus; beim Einschalten erst der Datenschutz-Hinweis; ohne Provider „not available“). Der
  gerätelokale Schalter „AI capture“ entfällt; `CapturePreferences` merkt nur noch den gelesenen Hinweis.
  Nach einem Wechsel ruft Kotlin `QuestApplication.profileSwitched()` (Widgets, dynamische Shortcuts).
  `CoreHolder.aiFeatures` meldet erst ab `CoreState.Ready` (kein Flackern von Share-Alias und Shortcut beim
  Kaltstart); `CaptureActivity` schließt mit Hinweis, wenn KI aus ist (alter Shortcut, direkter Aufruf). Ein
  fehlgeschlagener Gast-Import fragt mit Grund erneut. Browser-Seiten sammelt `MainActivity` nur, solange sie
  gestartet ist; der Channel ist Rendezvous, eine nicht abgeholte Seite (z. B. Logout im Hintergrund) verfällt.
- **desktop_peer:** `desktop_peer <data-dir> <server-url> <user>`; meldet sich als Desktop-Client über
  `quest_oidc_fake::Browser::authorize` an, Tokens in `<data-dir>/tokens.json` (`FileVault`).
- **Tests:** Rust `account_tests`, `vault_tests`, `core_tests` (Anmelde-URL, `NotASignIn`, `SignInExpired`),
  `tests/accounts.rs` gegen quest-server mit quest-oidc-fake (anna, Gast-Import, Abmelden mit Logout-URL, ben sieht
  nichts, annas Daten kommen zurück; KI-Opt-in vom Server gehalten). Kotlin `AccountViewModelTest`,
  `SettingsViewModelTest` (KI-Opt-in), Roborazzi `account-*` und `settings-ai-*`.
- Keycloak im Emulator nicht geprüft (kein Docker).

## Phase 12 (Android): App Link (seit Phase 21 nur noch im Core und Server)

- **Kein Server im APK:** Seit Phase 21 enthält kein Build einen Server-Host. Das Serverfeld startet in jeder
  Variante leer; angemeldet wird immer über das Private-Use-Scheme `app.quest.android:/oauth2redirect`.
  `RustQuestCore.beginSignIn` übergibt `app_link_origin = null`; `src/release/AndroidManifest.xml`,
  `PRODUCTION_ORIGIN` und `BuildConfig.APP_LINK_ORIGIN` sind entfernt.
- **Redirect-Wahl (Rust, bleibt):** `AppClient::android(server_url, app_link_origin)` in `quest-store` gibt
  `AppClient::AndroidAppLink` (Redirect `{origin}/auth/android`, `quest_protocol::android_app_link_redirect`)
  genau dann, wenn der Server `https` ist und sein Origin dem übergebenen App-Link-Origin entspricht (Host ohne
  Groß/Klein, Standard-Port, Pfad egal); sonst `AppClient::Android` (Private-Use-Scheme). Mit `None` immer
  Letzteres. Wer einen eigenen Build mit App Link baut, braucht dazu einen `autoVerify`-Filter für
  `{origin}/auth/android` und auf dem Server `QUEST_ANDROID_CERT_SHA256` (`assetlinks.json`).
- **Abmelden:** `return_to` bleibt immer `app.quest.android:/oauth2redirect`: die Rückkehr trägt kein Geheimnis,
  und Browser verlassen sich nach Redirect-Ketten ohne Tippen nicht zuverlässig in einen App Link.
- **Tests:** `login_tests` (Wahl inkl. Port, `http`, fremder Host, ohne Origin; App-Link-Ziel und abgelehnte
  Ziele; Logout), `core_tests` (Authorize-URL mit App Link, anderer Server mit Private-Use-Scheme).

## Phase 12: Kontraste (WCAG AA)

- **Ziel:** Text ≥ 4,5 : 1, UI-Komponenten (Linien, Punkte, Icons mit Bedeutung) ≥ 3 : 1, Desktop und Android
  gemeinsam über `crates/quest-app/assets/theme.json` (Android generiert daraus `QuestColors.kt`).
- **Hell geändert** (Werte gegen `bg` / ungünstigste Fläche `selected`):
  - `accent` (Palette `amber-deep`) `#c98f2a` → `#8c6014`: als Text 2,61 → 5,12 / 4,52; Weiß auf `accent`
    2,82 → 5,53. Gleicher Farbton (38°), dunkler und etwas satter. Ein Token statt getrennter Tokens für Fläche
    und Text: Weiß auf der Fläche braucht ohnehin ≥ 4,5 : 1, das erzwingt dieselbe Helligkeit wie Text.
  - Neues Token `error` (hell `#c1342d`, dunkel `#d9625c` wie bisher): 3,32 → 5,12 / 4,52. Desktop: Sync-Punkt
    „fehlgeschlagen“ (`topbar.rs` `tone_color`). Android: `StatusColors.failed` liest es jetzt aus dem Theme
    (`@Composable`-Getter), dazu `colorScheme.error`/`onError`.
  - `text-muted` `#6b6b6b` → `#686868`: auf `selected` 4,36 → 4,56.
- **Dunkel:** unverändert, alles ≥ 4,5 : 1 (kleinster Wert: `error` auf `hover` 4,59, `textMuted` auf `hover` 4,90).
- **Android, nur Farben:** Snackbar-Aktion nutzt `inversePrimary` (= Akzent der anderen Darstellung; vorher
  `accent` auf dem inversen Grund: dunkel 1,6 : 1). Swipe-Hintergrund: Label in `text` statt in der Aktionsfarbe
  (auf der 18-%-Tönung lag diese unter 4,5 : 1), Icon bleibt farbig (≥ 3 : 1).
- **Test:** `QuestColorsContrastTest` prüft streng (hell und dunkel): `text`, `textMuted`, `accent`, `error` auf
  allen Flächen (`bg`, `card`, `cardRaised`, `bar`, `chip`, `selected`, `hover`; auf `grid` ohne `error`) ≥ 4,5 : 1;
  `onPrimary`/`primary`, `card` auf `accent` (FABs, gefüllte Buttons), `onError`/`error`, Snackbar (`inverse*`),
  Swipe-Label auf der Tönung; Swipe-Icons ≥ 3 : 1. Widget-Vorschau (`res/values/colors.xml`) mitgezogen.
- **Bewusst ausgenommen:** `textFaint` (2,1–2,9 : 1; erledigte Aufgaben, Platzhalter, vergangene Tage – inaktiv),
  `border`/`borderStrong` (Dekoration), Quest-Farben (`quest_color`, nutzergewählt) und die Sync-Punkte
  `live`/`offline`/`busy`/`warn` (immer mit Text daneben).
- **Belege:** Roborazzi-Referenzen (`app/src/test/screenshots/`) und Desktop `crates/quest-app/snapshots/light*.png`
  neu aufgenommen.

## Phase 14 (Android): Task-Titel nur im Sheet, Umbenennen ohne Größensprung

- **Tasks:** Das Inline-Umbenennen auf den Karten (Home, Plan, Quest-Seite) entfällt wie am Desktop. Tippen auf
  den Titel öffnet wie die übrige Karte das Task-Sheet, in dem der Titel bearbeitet wird. Weg sind
  `Renaming`, `renaming`/`renamingTask` in `HomeInputs`/`PlanInputs`/`QuestDetailInputs`, die
  `beginRename`/`editRename`/`commitRename`-Aktionen und die Custom Action „Rename“. Focus behält sein
  Titelfeld (wie der Desktop-Focus).
- **Quest-Seite:** Der Name ist im Normalzustand die Überschrift (`QuestType.heading`, Desktop `ty::HEADING`).
  Tippen ersetzt sie durch `InlineTitleField` (`ui/components`): ein rahmenloses `BasicTextField` mit demselben
  Stil und denselben Modifiern, markiert nur durch eine gezeichnete Akzentlinie, also ohne Größen- oder
  Positionssprung. Done, Enter (Hardware-Tastatur), Ausblenden der Tastatur, Fokusverlust oder Verlassen der
  App bzw. Seite speichern (`QuestDetailViewModel.beginRename`, Entwurf in `QuestDetailInputs.title`). Leer
  angezeigt wird der Platzhalter „Name“. Meilensteine benennen sich nach demselben Prinzip um (`QuestType.body`,
  gleiche Box wie der Titel).
- **Tests:** `HomeInteractionTest.tappingATitleOpensTheTaskSheetNotAnInlineField`, `QuestRenameLayoutTest`
  (Stil, Grundlinie, Textanfang und -breite von Überschrift/Meilenstein gleich dem Feld),
  `QuestDetailInteractionTest` (Tippen auf Name/Meilenstein → Feld mit Fokus, Enter speichert),
  `InlineTitleFieldTest`, `QuestDetailViewModelTest.renamingTurnsTheNameIntoAFieldUntilSaved`; Roborazzi `quest-detail-renaming` und
  `quest-detail-renaming-milestone`.

## Phase 17: Quests wie am Desktop (Phasen 15 und 16)

- **Rust (`quest-mobile`):** `quests(now)` baut die Übersicht aus `quest_view::quests` (`overview`, `listed`,
  `count_label`): Kennzahlen `all_time_hours`, `in_progress`, `completed_count` („02“), Tab-Labels
  „Active · n“/„Completed · n“, Karten mit Farbe, Name, offenen Aufgaben, Wochen- und Gesamtstunden
  (Trend und Meilensteine auf der Karte entfallen), `new_quest_hues` (Farbe einer neuen Quest vorgewählt,
  `new_quest_color`). `quest_detail` hat `badge` („Completed“/„Archived“), `summary`
  („4.5h this week · 62h total · 2 open tasks“) und `status_action` „Complete quest“/„Reopen quest“/„Unarchive“.
  Meilensteine nur lesend (`MilestoneRow` mit `date_label`, `a11y_label`).
- **API:** `create_quest(title, description, hue?)` (eine Batch), `toggle_quest_status(quest, now)`
  (Active → Completed, Completed/Archived → Active; Zeit bleibt, Undo-Toast „Completed quest “…”“ wie am
  Desktop über `UndoEvent::QuestStatusChanged`), `unarchive_quest(quest)` (nur `Archived`, ohne Undo).
  `archive(now)` listet `archived_quests` („Archived quest“) getrennt vom Papierkorb. Entfernt:
  `complete_quest`, `reopen_quest`, `add_milestone`, `rename_milestone`, `set_milestone_date`,
  `delete_milestone`, `UnknownMilestone`.
- **quest-view:** die tote Meilensteinpflege ist raus: `TaskAction::MarkMilestone`/`mark_milestone`,
  `Step::Run`, Palette `Mode::MilestoneDate`/`Action::SetMilestoneDate`, `Toast::Completed`,
  `UndoEvent::Completed`/`MarkMilestone`, `UndoEffect::MarkMilestone` (quest-app nur angepasst, nicht verändert).
- **Android Quests-Tab:** Kennzahlen-Leiste, segmentierte Tabs mit Zählern aus dem Core, „Archive“ als eigener
  Button ohne Zähler (bricht schmal unter die Tabs um), Hinweis je Tab, leere Zustände je Tab, Karten im
  Desktop-Layout, gestrichelte „New quest“-Karte. Auf Telefonen zusätzlich der FAB „New quest“, ab 600 dp der
  Button im Kopf.
- **Quest-Editor** (`QuestEditorViewModel`, `QuestEditorSheet`): Tippen auf eine Karte öffnet ein Bottom Sheet
  über der Übersicht (Desktop: modaler Dialog) mit Titel, Beschreibung (Markdown wie im Task-Sheet), Farbe,
  Complete/Reopen quest, „Open quest page“, Kennzahlen und Delete. Speichern bei Done, Fokusverlust und
  Schließen (Desktop speichert live). „New quest“ öffnet es leer; „Create quest“ legt die Quest erst mit
  Titel an, danach bearbeitet das Sheet sie. Schließen lässt Tab und Scrollposition stehen (die Übersicht
  bleibt komponiert). Entwürfe überleben Prozess-Tod (`SavedStateHandle`).
- **Quest-Seite:** Arbeitsansicht ohne Beschreibung und ohne Meilensteinpflege; Meilensteine nur, wenn es welche
  gibt, nur lesend. Name umbenennen inline wie bisher, Badge bei Completed/Archived.
- **Archiv:** Abschnitt „Archived quests“ mit „Open“ und „Unarchive“ vor dem Papierkorb.
- **Tests:** Rust `core_quests_tests` (Abschließen/Wiedereröffnen behält Stunden, Undo, Unarchive, Anlegen mit
  Farbe/Beschreibung), Android `QuestsFlowTest` (Active → Completed → Active mit Zählern und Stunden, Undo in
  der Snackbar, Editor schließen behält Tab und Scroll, New quest nur mit Titel, Archiv mit Unarchive, Open
  quest page), `QuestEditorViewModelTest`, `QuestDetailInteractionTest` (Meilensteine ohne Klick, kein
  Abschnitt ohne Meilensteine, kein „Mark as milestone“). Roborazzi `quest-editor`, `quest-editor-new`,
  `quest-editor-completed`, `quest-detail-no-milestones`; entfallen `new-quest-sheet`,
  `quest-detail-renaming-milestone`.

## Phase 18 (Android): Workspaces

- **Rust (`quest-mobile`):** `workspaces(now)` liefert `WorkspacesData` (gezeigter Workspace, live mit
  Zusammenfassung aus `quest_view::workspaces::content_summary`, „Current · …“ beim gezeigten, `can_delete`,
  gelöschte, `timer_elsewhere` mit Name, Aufgabe und Zeit). `switch_workspace`, `create_workspace` (prüft den
  Namen wie der Desktop: getrimmt, ≤ 60 Zeichen, eindeutig; zeigt den neuen), `rename_workspace`,
  `delete_workspace_question` („Delete “Work”? It holds …“), `delete_workspace`, `restore_workspace` (abgelehnt,
  solange ein lebender Workspace den Namen trägt). `CoreState.Ready.workspaces` kommt mit jedem Publish.
- **Wechsel-Semantik wie am Desktop:** Der Core vergisst beim Wechsel (auch beim Rückfall, wenn ein anderes Gerät
  den gezeigten löscht; erkannt in `workspaces()`) Undo, KI-Undo, Plan-Vorschau und Tick-Gedächtnis
  (`follow_workspace`). Die App (`AppViewModels.leaveWorkspace`) speichert offene Sheets in ihren Workspace und
  schließt sie (Task, Quest-Editor, Quest-Seite → Quests-Tab, Log work/Eintrag), leert Quest-Filter und -Pick von
  Home und Plan und vergisst die Fokus-Aufgabe. Tab, Suche, Account, Settings und Archiv bleiben; getippter
  „Add …“-Text bleibt und landet im dann gezeigten Workspace.
- **Timer:** einer pro Person. `running_session` liefert den Timer auch aus einem anderen Workspace
  (`Store::snapshot_in`), mit `workspace_name`; die Benachrichtigung zeigt „<Quest> · in <Workspace>“.
  Pause/Resume/Complete aus der Benachrichtigung wirken auf die Aufgabe in ihrem Workspace
  (`Store::dispatch_in`, `QuestCore::task_anywhere`; die Domain plant Befehle sonst gegen den gezeigten
  Workspace). Complete in einem anderen Workspace setzt keinen Undo-Toast. Ein pausierter Hinweis bleibt, solange
  die Aufgabe irgendwo offen ist (`is_task_open`).
- **UI:** Switcher (Ebenen-Icon, Name, Chevron) ersetzt den Titel in der Top-Bar (Telefon zu schmal für beides;
  Quests trägt seine Überschrift jetzt im Inhalt wie Review). Menü: lebende Workspaces mit Haken, Trenner,
  „Manage workspaces…“. „Manage workspaces“ ist ein Bottom Sheet (Desktop: Dialog): je Workspace Name mit Rename,
  Zusammenfassung, Delete (der letzte ist gesperrt); die Bestätigung ersetzt die Zeile und nennt den Umfang;
  „New workspace“ + Create (zeigt den neuen, schließt das Sheet); „Deleted“ mit Restore. Eingaben überleben
  Prozess-Tod (`SavedStateHandle`). Läuft der Timer anderswo, steht unter der Top-Bar und im Fokus-Kopf
  „● 36:00 in Default“; Tippen wechselt dorthin und öffnet Fokus. Suche: „Switch workspace…“ und
  „Manage workspaces…“ wie die Palette (12 Einträge wie am Desktop; leer zeigt sie nun nur Befehle).
- **Widget, Kachel, Shortcuts:** folgen ohne eigenen Code dem gezeigten Workspace: das Widget zeichnet bei
  `core.changes` neu (ein Wechsel meldet `on_change`), Kachel/Quick-Add legen im gezeigten an, der Shortcut
  „Start timer“ wird beim App-Stop neu veröffentlicht. Die Auswahl merkt sich der Store pro Gerät und Profil.
- **Tests:** Rust `core_workspaces_tests` (9), `core_search_tests`, Store `tests/workspaces.rs`
  (`dispatch_in`/`snapshot_in`); Android `WorkspacesViewModelTest`, `WorkspaceSwitchTest` (Menü-Wechsel,
  Quest-Seite weicht beim Rückfall, Manage mit Lösch-Bestätigung und Create, Suche öffnet Manage, Timer-Hinweis
  → Fokus), `FocusNotificationsTest` (Workspace im Text). Roborazzi `workspace-timer-elsewhere`,
  `workspace-menu` (neue Art `SceneKind.Window`, nimmt das Popup mit), `workspaces-manage` (auch 200 %),
  `workspaces-delete`, `focus-timer-elsewhere`; alle Szenen mit Top-Bar neu aufgenommen.

## Phase 20 (Android): Quest-Picker

- **Eine Komponente** (`ui/components/QuestPicker.kt`) für jede Quest-Auswahl: Auslöser bleibt der Chip
  (`QuestPickerChip`) bzw. das Filter-Icon (`QuestFilterButton`), Fokus-Pille und das „Assign quest“-Icon der
  Karte öffnen dasselbe `QuestPickerSheet`. Form: Bottom Sheet (nur ganz offen) mit Suchfeld oben (Fokus und
  Tastatur beim Öffnen, „Search quests…“), darunter die Liste (Farbpunkt, Titel, Haken bei der gewählten), die
  „keine“-Zeile („No quest“ bzw. im Filter „All quests“) **zuletzt** wie am Desktop, auch per Suche erreichbar;
  Done der Tastatur nimmt die erste passende Zeile (Desktop: Enter). Leer: „No quests match“. Die Liste ist für
  die ungefilterte Anzahl hoch (max. 360 dp, scrollt), damit das Sheet beim Filtern nicht springt.
- **Sortierung** „zuletzt genutzt“ kommt fertig aus Rust (`quest_view::quest_picker::recent_quests` über alle
  `questOptions`/`quests` von `quest-mobile`); Kotlin sortiert nie. Gefiltert wird in Kotlin mit einer Zeile wie
  `quest_view::quest_picker::matches` (Teilstring, ohne Groß-/Kleinschreibung, Query getrimmt), weil
  Robolectric keine native Lib lädt.
- **Stellen:** Home (Chip-Reihe über „Add quest work…“ entfällt; wie am Desktop Filter-Icon im Kopf von QUEST
  WORK, der Filter-Chip daneben löscht ihn, der Chip im Eingabefeld wählt die Quest neuer Arbeit), Task-Sheet,
  Log work (neue Aufgabe), Plan-Filter, Fokus, Quick-Add, AI-Capture (Aufgabe und neue Aufgabe einer Session;
  „New: …“-Quests bleiben Zeilen), „Assign quest“ auf Karten. Keine „Create quest“-Zeile: keine Stelle legt
  heute Quests an. Workspace-Switcher unverändert.
- **Tests:** `QuestPickerTest` (öffnen, Haken, filtern, wählen, „No quest“ zuletzt und per Suche, ohne
  „keine“-Zeile, Leerzustand, Done), `HomeInteractionTest` (Filter-Icon und Feld-Chip), `LogWorkQuestTest`.
  Roborazzi `quest-picker`, `quest-picker-filtered`, `quest-picker-empty` (15 Quests), `home-filtered`.

## Offen

- AI-Erfassung: `dispatch_all` baut den Snapshot pro
  Command neu (bei sehr großen Stores und vielen Einträgen spürbar); das Rate-Limit zählt auch abgelehnte
  Anfragen.

- Startet ein anderes Gerät den Timer, während der App-Prozess eingefroren ist, erscheint die Notification
  erst beim nächsten App-Start (kein Push; FCM würde einen FGS-Start erlauben).
- Tagesraster: Sommerzeit-Tage zeigen Sitzungen und Blöcke mobil in Wanduhr-Minuten (seit Phase 4, damit Drags
  dieselben Minuten zurückschreiben); der Desktop zählt Sitzungen weiter in vergangenen Minuten.
- Release-Schlüssel: Ablage des echten Schlüssels und der Secrets ist offen (Frage); bis dahin baut die CI nur
  mit gesetzten Secrets.
- Plan: keine Backlog-Spalte und keine „Recurring tasks“-Liste; Backlog nur über „Move to…“.
- Plan-Board: nach seitlichem Auto-Scroll bleibt das Board zwischen zwei Spalten stehen (Snap nur beim Fling).
- Plan: auf leere vergangene Tage kann man nicht ziehen (sie haben weder Karten noch Add-Feld als Ziel).
- Calendar: Long-Press auf einen nicht beweglichen Block (laufende Sitzung) wirkt wie ein Tipp.
- Calendar: sehr kurze Blöcke zeigen bei großer Schrift nur den Titel, einzeilig gekürzt (Höhe = Dauer); TalkBack liest das volle Label.
- Quests: Meilensteine haben keinen Erledigt-Zustand und lassen sich (wie am Desktop seit Phase 16) nirgends
  mehr anlegen oder pflegen; alte bleiben lesbar.
- Quests: Bei 200 % Schrift kürzt der Tab „Completed · n“ auf Telefonen mit „…“ (TalkBack liest das volle Label).
- Suche: `Choose` und `Done` geben außer Moduswechsel bzw. Schließen keine Rückmeldung.
- Widget: Stunden ändern sich nur, solange der Core tickt (App sichtbar oder Timer läuft) bzw. bei Änderungen,
  Tageswechsel, Worker-Lauf; hell/dunkel folgt dem System, nicht der App-Einstellung; keine Dynamic Color.
- Kontraste: `textFaint`, Rahmen, Quest-Farben und Sync-Punkte (außer „fehlgeschlagen“) liegen unter AA
  (bewusst ausgenommen, siehe Phase 12: Kontraste).
- TalkBack-Durchlauf nur über den Accessibility-Baum belegt; Gesten lassen sich per adb nicht einspielen.
- Toolchain (Review N2): kein `mise.lock` mit Checksummen, `jdx/mise-action` ohne feste mise-Version, `cargo-ndk`
  je nach mise-Einstellung per binstall oder `cargo install` ohne `--locked`.
- `UrlPolicy` gilt beim Anmelden, nicht für eine gespeicherte Sitzung. Eine 10.0.2.2-Sitzung kann im Release nur
  aus einem Debug-Build stammen; Debug und Release sind verschieden signiert, Android installiert das eine nicht
  über das andere, und Deinstallieren löscht die Daten. Deshalb kein Check beim Öffnen. Die App hat keinen Weg,
  lokale Daten eines anderen Kontos zu verwerfen (die Core-Meldung empfiehlt es); bis dahin „Speicher löschen“.
- Ein erzwungener periodischer Job (`cmd jobscheduler run -f`) vor seinem Termin wird von WorkManager
  verschoben; der Prozessstart synct dann trotzdem live (Core öffnet im `Application.onCreate`).
- AI-Erfassung: Dialoge über dem Sheet (Datenschutz, Mikrofon) haben dieselbe Fläche wie das Sheet und heben
  sich kaum ab; Blöcke und Wiederholungen im Vorschlag lassen sich nicht neu anlegen; Live-Diktat nur auf echten
  Geräten prüfbar.
- Unbekannte Task-Ids (Widget auf gelöschte Aufgabe) enden in der Core-Meldung (InvalidId), ohne Sheet. Ein
  Request (Deep Link, Shortcut) geht verloren, wenn die Activity vor dem Öffnen des Cores rotiert.
