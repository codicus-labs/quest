# Plan: Android-App komplett (Design und Umsetzung)

Grundlage ist der Prototyp aus `docs/mobile.md`: `quest-mobile` (UniFFI) über `quest-store`, dazu Compose in
`apps/android`. Zuletzt entschieden wurde Folgendes:

- Quest-Look auf Material 3.
- Alle Desktop-Funktionen, fürs Handy umgebaut.
- Focus-Notification, Widget, Hintergrund-Sync, Teilen/Quick-Add.
- Phone first, adaptiv für Tablet.

## Architektur

```
Compose-Screens ── ViewModels (Kotlin, dünn: StateFlow, Navigation, Plattform-APIs)
   └─ QuestCore (UniFFI, quest-mobile): Ansichtsdaten + Aktionen, eine Instanz pro Prozess
       └─ quest-view (NEU, pur): Präsentationslogik, die Desktop, Android und iOS teilen
           └─ quest-domain (Policy)          quest-store (SQLite, Sync), unverändert
```

**Präsentationslogik:** Sie liegt in Rust und nicht in Kotlin. Viel davon steckt heute in `quest-app`, ohne
Rune-Bezug: `format`, `timescale`, `log_work`, `entry_editor`, `undo`, `plan_strip`, `palette` (Suche) und `menu`.
Diese Module ziehen in eine neue pure Crate `quest-view`. `quest-app` nutzt sie danach per Re-Export,
das Verhalten bleibt unverändert. `quest-mobile` bildet daraus UI-Records. Kotlin rechnet keine Zeiten,
Stunden, Labels oder Validierungen. iOS bekommt das später gratis.

**Kern-API:**
- Pro Ansicht eine Abfrage, z. B. `home(now)`, `plan(range, now)`, `calendar(day, now)`, `review(week, now)`.
- Pro Aktion ein Aufruf, z. B. `add_task`, `move_task`, `log_work`, `edit_entry`, `start_timer`.
- Fehler sind flache Codes mit Meldung.
- Alles läuft im Speicher. Nur `open` macht IO und läuft auf `Dispatchers.IO`.

**Prozessmodell:**
- `QuestCore` ist ein Singleton im `Application`-Objekt.
- Activity, Widget, Foreground-Service, WorkManager und Share-Target teilen sich dieses Objekt.
- Der `onChange`-Callback stößt einen geteilten `StateFlow` an.

**Zeit:**
- `Moment` kommt von der Plattform: Epoch und IANA-Zone.
- Ein Ticker im Core-Holder weckt die App nur an Minuten- und Mitternachtsgrenzen, ein laufender Timer zusätzlich
  jede Sekunde. Das ist dieselbe Regel wie in Desktop-Runde 9.
- `GenerateDueTasks` läuft beim Start, um Mitternacht und minütlich, solange die App im Vordergrund ist.

**TLS:** Die App initialisiert `rustls-platform-verifier-android` beim Start und reicht JVM und Context an
`quest-mobile` weiter. Die Server-URL wird in `quest-mobile` geprüft: `http` ist nur für `10.0.2.2`/Loopback
erlaubt.

## Design-System (Quest-Look)

**Tokens:**
- Die Farben kommen aus `crates/quest-app/assets/theme.json`, hell und dunkel. Ein Gradle-Task erzeugt daraus
  `QuestColors.kt`, damit es eine Quelle gibt.
- Die Quest-Akzentfarben liefert der Core als Hex.
- Typografie und Abstände werden aus `theme.rs` übertragen.

**Komponenten:**
- `TaskCard`: Haken, Titel, Quest-Chip, Timer-Knopf und erfasste Dauer. Swipe nach rechts erledigt die Aufgabe,
  nach links löscht sie. Long-Press zieht die Karte.
- `QuestChip`.
- `HoursHeader` mit Wochenziel-Balken.
- `SectionHeader` für Quest Work und Other.
- `TimeBlock` im Raster.
- Bottom Sheets statt Dialogen.
- `UndoSnackbar`.
- Leere Zustände.

**Navigation:**
- Bottom Bar mit Home, Plan, Calendar, Quests und Review. Auf Tablets ist es eine Navigation Rail.
- Focus ist ein Vollbild über allem und wird von der Karte oder der Notification geöffnet.
- Suche (statt ⌘K) und Account sitzen oben rechts.

**Adaptiv:** Die Window Size Classes bestimmen das Layout:

| Breite | Home | Plan | Calendar | Review |
|---|---|---|---|---|
| compact | Liste, darunter das Tagesraster als Tab oder Sheet | vertikale Tagesliste | Tagesraster, Wischen zum nächsten Tag | eine Spalte |
| medium und expanded | Liste und Tagesraster nebeneinander | Spalten wie am Desktop, horizontal endlos | 3 bzw. 7 Tage | zwei Spalten |

## Phasen

Jede Phase gilt als fertig, wenn:
- die Rust-Gates grün sind;
- `./gradlew assembleDebug testDebugUnitTest` grün ist;
- die Screenshot-Tests (Roborazzi, hell, dunkel und Tablet) grün sind;
- der Ablauf im Emulator geprüft ist (`adb shell input` und Screenshots);
- `docs/mobile.md` ergänzt ist.

### Phase 0: Fundament

- `quest-view` extrahieren: Modul für Modul verschieben, die `quest-app`-Tests bleiben grün, und `quest-app`
  ändert nur Imports.
- `QuestCore` als Application-Singleton aufsetzen, mit Ticker und `GenerateDueTasks`.
- HTTPS über den Verifier und die URL-Policy.
- Theme-Codegen, die Komponenten-Grundlagen, Navigation (Bottom Bar oder Rail) und den Screenshot-Test-Aufbau.
- Account- und Settings-Screen: Server, Login und Registrierung, Abmelden, Wochenziel, Darstellung
  (System/Hell/Dunkel).
- **Fertig:** Der Prototyp läuft auf dem neuen Fundament, und HTTPS gegen einen TLS-Server ist im Test belegt.

### Phase 1: Home und Task

- Quest Work und Other, Quest-Filter und Other ausblenden.
- Aufgaben oben oder unten erfassen. Ist ein Filter aktiv, bekommt die Aufgabe dessen Quest; sonst hängt der Chip
  der zuletzt genutzten Quest daran (Regel vom Desktop).
- Timer an der Karte, Swipe-Aktionen, Undo, Inline-Umbenennen.
- Task-Sheet (entspricht dem großen Task-Modal): Titel, Beschreibung, Quest, Datum, Wiederholung, Zeithistorie,
  Löschen.
- Kopfzeile mit Quest Hours für heute und die Woche.

### Phase 2: Focus

- Vollbild mit Titel, Quest, großem Timer, Pause und Fortsetzen, Complete (danach zurück zu Home) und bearbeitbarer
  Beschreibung.
- Ein Foreground-Service (`specialUse` bzw. passender Typ) mit fortlaufender Notification: Chronometer, Pause und
  Complete.
- Der Service läuft weiter, wenn die App geschlossen ist. Er endet, sobald die Sitzung endet, auch wenn sie auf
  einem anderen Gerät beendet wurde (über Sync).
- Die Berechtigung `POST_NOTIFICATIONS` wird beim ersten Start des Timers abgefragt.

### Phase 3: Plan

- Compact: endlose Tagesliste rund um heute (`LazyColumn` mit Tagessektionen).
  - „Heute“-Sprung und Datumswahl.
  - Long-Press zieht eine Karte auf einen anderen Tag, mit Auto-Scroll am Rand.
  - Hinzufügen direkt am Tag.
- Medium und expanded: horizontale Tagesspalten. Die Fenster-Logik kommt aus `quest-mobile::plan` (in Tagen, nach
  dem Vorbild von `quest-view::plan_strip`), die Rand-Geschwindigkeit aus `plan_strip::edge_speed`.

### Phase 4: Calendar, Log work, Zeiteinträge

- Tagesraster über 24 h mit Blöcken, der Now-Line und Tagessumme.
- Wochenkopf mit Quest Hours gegen das Wochenziel.
- Long-Press und Ziehen erzeugen einen Zeitraum und öffnen „Log work“. Wie am Desktop wird erst dort gespeichert.
- Das Log-work-Sheet bietet Datum, Start und Ende mit nativen Pickern, eine vorhandene Aufgabe (deren Quest gilt)
  oder eine neue erledigte Aufgabe.
- Entry-Editor:
  - Aufgabe wechseln, Start und Ende (über Mitternacht), Dauer, Speichern und Löschen.
  - Die Quest ergibt sich immer aus der Aufgabe.
- Blöcke lassen sich verschieben und in der Länge ändern. Das öffnet den Editor nicht.

### Phase 5: Quests, Review, Archiv, Suche

- Quests:
  - aktive und abgeschlossene Quests mit Zeit pro Quest;
  - Detailansicht mit Titel, Beschreibung, Farbe, Symbol, Milestones und „Complete quest“.
- Review:
  - Quest Hours gegen das Ziel, Tagesdurchschnitt, Zeit pro Tag und Quest;
  - Quest Work im Verhältnis zu Other;
  - Verlauf über mehrere Wochen (Charts mit Compose Canvas, keine Chart-Bibliothek).
- Archiv: Gelöschtes wiederherstellen.
- Suche: Aufgaben, Quests und Aktionen. Die Logik kommt aus `quest-view::palette`.

### Phase 6: Plattform

- **Widget** (Glance):
  - Zeigt die Aufgaben von heute und die Quest Hours. Abhaken geht direkt im Widget, Tippen öffnet die App.
  - Es aktualisiert sich über `onChange`.
- **Hintergrund-Sync** (WorkManager):
  - Ein periodischer Job (15 min, nur mit Netz) öffnet den Core bei Bedarf, synchronisiert bis Ack bzw. Timeout und
    aktualisiert das Widget.
  - Dafür braucht `quest-store` eine Abfrage oder ein Signal „Outbox leer und Catch-up fertig“.
- **Teilen und Quick-Add:**
  - Share-Target für Text wird zu einer neuen Aufgabe (Sheet mit Quest und Datum).
  - App-Shortcuts „Neue Aufgabe“ und „Timer starten“.
  - Quick-Settings-Kachel „Neue Aufgabe“.

### Phase 7: Produktreife

- Barrierefreiheit (TalkBack-Labels, Schriftgröße 200 %, Kontraste).
- Baseline-Profile und Startzeit.
- Größe der `.so`:
  - eigenes Release-Profil mit LTO, `opt-level="z"` und `panic=abort`;
  - prüfen, ob `ring` statt aws-lc reicht;
  - zusätzlich das ABI `x86_64` für Emulatoren auf Intel.
- Release-Signing.
- Ein Crash-Handler für Rust-Panics: Panics werden zu Fehlern statt zum Abbruch.
- Umgesetzt (Details in `docs/mobile.md`, Phase 7): Profil `mobile-release` mit LTO und `opt-level="s"` (gleicher
  Download wie `"z"`, schnellerer Code); `panic` bleibt `unwind`, weil der Crash-Handler (`guard`) sonst nicht greift.
  `ring` statt aws-lc. Signiert wird in der CI außerhalb von Gradle; verteilt per GitHub-Release (Obtainium).

## Ausführung

- Phase 0 läuft allein, weil alles darauf aufbaut.
- Danach laufen die Phasen 1 → 2 → 3 → 4 → 5 nacheinander. Jede Phase ist ein eigener Helfer-Auftrag mit Review
  und Gates durch mich.
- Phase 6 kann ab Phase 2 parallel laufen: Widget und Sync brauchen nur Home und Focus.
- Bevor ein Helfer eine Phase beginnt, liegen ihm deren Screens als Compose-Skizzen bzw. Screenshots vor.
  Der Design-Review findet am ersten Screenshot-Satz statt, nicht erst am Ende.

## Entscheidungen

1. **Extraktion aus `quest-app`:** Zugestimmt. Der Umzug nach `quest-view` ändert kein Verhalten.
2. **Quest-Beschreibung:** Klartext, wie am Desktop.
3. **Hintergrund-Sync:** Zugestimmt. `quest-store` bekommt eine kleine Erweiterung „sync until idle“.
4. **Ziel-API:** minSdk 31 (Android 12) und targetSdk 37.
5. **Phase 8, KI-Erfassung:**
   - Diktieren bzw. Text wird über eine externe KI zu Tasks, Quests und Zeiten strukturiert. Der User bestätigt den
     Vorschlag.
   - Der Provider ist generisch und OpenAI-kompatibel (OpenRouter, OpenAI). Schlüssel und Aufruf liegen in
     `quest-server`.
   - Läuft auf Android und Desktop. Erkannt wird alles, was das Datenmodell hergibt. Keine Schätzungen:
     Tracking statt Schätzen.
6. **Phase 9, MCP-Server:**
   - Remote in `quest-server` (Streamable HTTP, `/mcp`), Auth per OAuth 2.1 mit PKCE.
   - Lesen und Schreiben über dieselben Domain-Commands.
