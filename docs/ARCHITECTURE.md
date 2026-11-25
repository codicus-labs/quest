# Quest (Rust) – Architektur

Quest komplett in Rust; der macOS-Client nutzt das UI-Framework
[Rune](https://github.com/anomalyco/rune) (Anomaly/OpenCode) und dient zugleich dessen Evaluation.

## Produkt

- **Quest**: langfristiges Ziel mit Titel, Farbe und Status. **Task**: konkrete Arbeit, optional einer Quest
  zugeordnet, mit optionalem Plan-Datum (`planned_on`) und optionaler Deadline (`due_on`, Plan ≤ Deadline) und
  einer Markdown-Beschreibung. **Checklist item**: abhakbarer Punkt eines Tasks. **Postponement**: eine Verschiebung
  von Plan-Datum oder Deadline auf später. **Time entry**: getrackte Zeit, Art `Task` (Focus-Session auf einem Task;
  Zeit auf Quest-Tasks = „Quest Hours“) oder `Organizing` (ohne Task, optional mit Herkunfts-Task).
  **Settings**: Wochenziel in Minuten. Begriffe: `GLOSSARY.md`.
- Listen (Regeln pur in `quest-domain/src/dates.rs`, Abfragen in `quest-view/src/lists.rs`): **Today** (offen und
  Plan oder Deadline ≤ heute; überfällig, liegengeblieben, heute fällig, Rest), **Inbox** (offen, ohne Quest und
  ohne Plan-Datum), **Logbook** (erledigt, neueste zuerst). Nur der User verschiebt Daten.
- Desktop-Shell ohne Top-Navigation: links die Sidebar (Workspace-Wechsler, **Inbox**, **Today**, **Plan**, **Review**,
  **Logbook** mit Zählern, aktive Quests mit Stundenring, abgeschlossene Quests, neue Quest, Player-Karte mit dem
  laufenden Timer oder Organizing), in der Mitte die Ansicht, rechts die Spalte „Tracked today“ (Timeline, ein Block
  öffnet den Entry-Editor) und „This week“ (ein-/ausblendbar mit `T` und ⌥⌘I). Dazu **Focus** (ein Task, großer
  Timer), **Quests** und die Seite einer Quest. **Plan** zeigt Spalten pro Tag nach Plan-Datum, **Review** die Woche
  (Stunden pro Quest, Organizing-Anteil; Verschiebungen, liegengebliebene und überfällige Tasks aus `WeekTasks`).
  Was Sidebar, Zeilen, Timeline und Player zeigen, rechnet `quest-view` (`nav`, `lists`, `row_meta`, `tracked`,
  `player`).
- ⌘K Command-Menü („Search or jump…“), Tastatur-first, Light/Dark.
- Offline-First: Alles funktioniert ohne Server und ohne Konto (Gast); Sync, sobald angemeldet und verbunden.

## Crates

| Crate | Rolle | Abhängigkeiten |
|---|---|---|
| `quest-domain` | Entities, HLC, LWW-Merge (`FieldStore`), `Command` → `plan()` → `FieldWrite`s, `Snapshot`, Datumsregeln (`dates`), `WeeklyReview`/`WeekTasks`. Pur, kein IO. | chrono, serde, uuid |
| `quest-protocol` | Auth-Pfade und -DTOs (`AccountInfo`, `TokenResponse`, Erstanbieter-Clients, Scopes), WebSocket-Nachrichten, Pfade, KI-Erfassung (`CaptureRequest`, `CaptureProposal`). | quest-domain |
| `quest-capture` | KI-Erfassung, pur: Kontext und Request, Prompt und JSON-Schema, strikte Validierung der Modellantwort, Review-Entwurf und Labels, bestätigter Vorschlag → Domain-Commands und Undo. Keine IO, keine Uhr. | quest-domain, quest-protocol, quest-view |
| `quest-server` | axum-HTTP + WebSocket, SQLite via sqlx; Anmeldung per OIDC beim Identity Provider, OAuth-2.1-Authorization-Server für Apps und MCP (Bearer-Token mit Audience). | tokio, axum, sqlx, openidconnect |
| `quest-oidc-fake` | OIDC-Provider für Tests und lokale Entwicklung (Discovery, JWKS, RS256-ID-Tokens, absichtliche Fehler) plus Test-Helfer für einen kompletten App-Login ohne Browser (`app_login`, `Browser`). Nie für Produktion. | axum, rsa, reqwest |
| `quest-ai-eval` | Eval-Set der KI-Erfassung: synthetische Diktate mit erwarteten Vorschlägen, Bewertung (pur), Binary gegen ein echtes Modell (`mise run ai-eval --model <id>`, siehe `docs/ai-models.md`). | quest-capture, quest-server |
| `quest-store` | Client-Persistenz (SQLite, rusqlite) + Sync-Engine (tokio-tungstenite), Profile (Gast + ein Store pro Konto), App-Login (OAuth 2.1, PKCE) und Token-Speicher. UI-agnostisch. | rusqlite, tokio, reqwest |
| `quest-view` | Präsentationslogik ohne UI-Framework, geteilt von den Clients: `format`, `lists`, `log_work`, `entry_editor`, `undo`, `plan_strip`, `quests`, `palette`, `menu`, `nav` (`View`). Pur, kein IO, keine Uhr (Zeit wird übergeben). | quest-domain, chrono |
| `quest-app` | macOS-Client mit Rune + rune-kit. | rune, quest-store, quest-view |
| `quest-mobile` | UniFFI-Schicht für native Mobile-UIs (Android-Prototyp in `apps/android`, siehe `docs/mobile.md`). | uniffi, quest-store |

## Sync-Modell: Field-Level Last-Writer-Wins

- Einheit ist `FieldChange { kind, id, field, value, hlc }`. Pro (Entity, Feld) gewinnt der höchste
  HLC. Merge ist kommutativ, assoziativ, idempotent → Replikate konvergieren unabhängig von
  Reihenfolge und Wiederholungen. Löschen = Soft-Delete über das Tombstone-Feld `deleted_at` (Zeitstempel); `Restore` setzt es zurück, gelöschte Entities erscheinen im Archiv.
- Neue Aufgaben erhalten offline erzeugte UUIDv7. Sortierung per Fractional Index (`sort_key`).
- Die Wiederholungsregel liegt auf der ursprünglichen Aufgabe und verwendet Kalenderdaten in
  ihrer IANA-Zeitzone. Ein wiederholter Termin erhält eine UUIDv5 aus Aufgaben-ID und Datum.
  Geräte können denselben Termin offline erzeugen. Seine Anfangswerte liegen in einem
  separaten Feld, sodass spätere Benutzereingaben beim Merge Vorrang behalten.
- **Server** ist autoritativ für die Reihenfolge: Jede gewinnende Änderung bekommt eine pro User
  monoton steigende `seq`. Speicherung: eine Zeile pro (user, kind, id, field) mit value, hlc, seq –
  kein separates Log nötig, Catch-up = `WHERE seq > cursor ORDER BY seq`.
- **Client** hält dieselbe Feld-Tabelle lokal + `outbox` (unbestätigte lokale Changes) + `meta`
  (node_id, cursor, Kontoinfo; Tokens nie in der DB). Lokale Commands werden sofort angewendet (optimistisch), in die
  Outbox geschrieben und bei Verbindung gepusht; nach `Ack` aus der Outbox entfernt.
- Protokolldetails: Doku-Kommentar in `crates/quest-protocol/src/lib.rs`.
- **Postponements** werden im selben Command wie die Datumsänderung geschrieben (`postponed_fields`), nie geändert;
  Undo löscht sie weich. Ihre ID ist eine UUIDv5 aus Task und Inhalt, weil `plan()` pur ist. Zwei Geräte, die
  offline verschieben, behalten beide Verschiebungen; das Datum selbst entscheidet LWW.
- Ein wiederholter Termin bekommt `planned_on` = Termindatum, keine Deadline und die Checkliste der Vorlage
  unabgehakt; jeder Punkt mit UUIDv5 aus Termin- und Punkt-ID und Anfangswerten im separaten Feld `initial`.

### Datenmodell 2: der Schnitt

Das Modell hat sich ohne Migrationspfad geändert (Kalender, Zeitblöcke, Meilensteine und Quest-Beschreibung
entfernt; neue Entities oben). Deshalb:
- `PROTOCOL_VERSION` 2: Clients mit Protokoll 1 bekommen `UnsupportedProtocol`.
- Server-Migration `0005_drop_synced_data.sql` löscht alle Feldzeilen, setzt `last_seq` auf 0 und vergibt jedem
  User eine neue Epoch. Konten, OAuth- und OIDC-Daten bleiben; Anmeldungen bleiben gültig.
- Client-Store: Meta `model` (aktuell `2`). Weicht es beim Öffnen ab, löscht der Store Feld-Tabelle, Outbox, Cursor,
  Epoch und den gemerkten Workspace. Profile, Node-ID und Kontoinfo bleiben, Tokens liegen ohnehin im Vault.
- So lädt niemand Altes wieder hoch: alte Clients sind abgewiesen, neue Stores sind leer, wenn sie die neue Epoch
  sehen.

### Workspaces (Phase 18)

- Ein Konto bzw. das Gastprofil hat beliebig viele **Workspaces** (`EntityKind::Workspace`, Felder `name`,
  `deleted_at`). Quests, Tasks/Serien, Checklist-Punkte, Postponements und Zeiteinträge tragen
  `workspace_id`; das Wochenziel ist der
  Settings-Datensatz mit der ID des Workspace. Konto, Anmeldung und Sync bleiben auf Kontoebene: ein Sync für alle
  Workspaces, das Feld synct wie jedes andere.
- **Default-Workspace** mit fester ID (`EntityId::DEFAULT_WORKSPACE`, die nil-UUID, zugleich die bisherige
  Settings-ID). Jedes Gerät kennt ihn ohne Absprache, auch offline. Er lässt sich wie jeder andere umbenennen,
  löschen und wiederherstellen. Gelöscht werden kann jeder Workspace, solange ein anderer lebender bleibt
  (`CannotDeleteLastWorkspace`). Löschen zwei Geräte offline zusammen alle, gilt der Default als nicht gelöscht.
  MCP-Aufrufe ohne Workspace meinen die feste Default-ID; ist der Default gelöscht, verweigert MCP sie.
- **Zuordnung** (reine Lesefunktion, `quest-domain/src/workspace.rs`): der eigene `workspace_id`, sonst der der
  Eltern-Entität (ein Termin folgt seiner Serie, ein kopierter Checklist-Punkt seinem Task), sonst der Default.
- `Snapshot::from_store(store, workspace)` liefert die Sicht genau eines Workspace. Ein gelöschter oder unbekannter
  Workspace fällt auf den Default zurück, ist der gelöscht, auf den ersten lebenden. Alle Views (Today, Inbox, Plan, Focus, Review, Logbook, Suche, Archiv) und der
  Kontext der KI-Erfassung lesen nur diese Sicht. Commands legen Neues im Workspace des Snapshots an. IDs aus anderen
  Workspaces kennen sie nicht (`UnknownTask` usw.). So kann ein veralteter Dialog nichts in einen anderen Workspace
  schreiben.
- Löschen ist ein Soft-Delete nur des Workspace-Datensatzes. Sein Inhalt bleibt unverändert und ist ausgeblendet, bis
  der Workspace wiederhergestellt wird. Offline-Änderungen anderer Geräte am Inhalt bleiben deshalb erhalten.
- Die Auswahl merkt sich jedes Gerät pro Profil in der `meta`-Tabelle des Profil-Stores (`Store::switch_workspace`).
  Löscht ein anderes Gerät den aktiven Workspace, zeigt der Store den Ersatz (siehe oben) und bleibt dort.
- **Focus-Timer global:** höchstens ein laufender Timer pro Person über alle Workspaces. `Snapshot::running` listet
  alle laufenden Sessions mit ihrem Workspace, `running_focus()` die maßgebliche, `running_entry()` nur, wenn sie im
  aktiven Workspace liegt. `StartFocus` und `StopFocus` beenden laufende Sessions in jedem Workspace;
  `DeleteWorkspace` beendet die in diesem Workspace.

### Vertrag `quest-store` (wird von `quest-app` und `quest-mobile` genutzt)

```rust
// Profile: Gast + ein Store pro Konto (data_dir/guest, data_dir/accounts/<key>, active.json). Alle Methoden
// blockieren (Disk-, teils Netz-IO): nie im UI-Thread. Sie liefern den nächsten Store; die App ersetzt ihren.
pub struct ProfilesConfig { pub data_dir: PathBuf, pub vault: Arc<dyn TokenVault> }
impl Profiles { // ein Profiles pro Datenordner und Prozess
    pub fn open(config: ProfilesConfig, on_change: impl Fn() + Send + Sync + 'static) -> Result<(Profiles, Store), StoreError>;
    pub fn sign_in(&self, current: &Store, signed_in: SignedIn) -> Result<SignInOutcome, StoreError>; // Switched { store, offer_guest_import } | Renewed
    pub fn import_guest(&self, account: &Store) -> Result<usize, StoreError>; // Gastdaten als neue Änderungen ins Konto
    pub fn sign_out(&self, current: &Store) -> Result<Store, StoreError>;     // Revoke, Tokens + Konto-DB löschen → Gast
}

// Login (RFC 8252): state + PKCE S256, Authorize-URL; finish prüft state/iss, tauscht den Code, holt das Konto.
impl LoginRequest {
    pub fn new(server_url: &Url, client: AppClient) -> Result<LoginRequest, StoreError>; // AppClient::Desktop { redirect_port } | Android
    pub fn authorize_url(&self) -> &Url;
    pub fn finish(&self, callback_url: &Url) -> Result<SignedIn, StoreError>; // blockiert
}

// Tokens nur hier, nie in der DB: KeychainVault (macOS, Feature `keychain`), FileVault (Dev/E2E), MemoryVault,
// Android über einen UniFFI-Callback (Keystore).
pub trait TokenVault: Send + Sync { fn load(..); fn save(..); fn delete(..); }

impl Store { // Send + Sync; als Arc<Store> zwischen UI- und Hintergrund-Threads teilen
    pub fn snapshot(&self) -> Arc<Snapshot>;                              // billig, nie blockierend (kein Disk-IO); aktiver Workspace
    pub fn switch_workspace(&self, id: EntityId) -> Result<(), CommandError>; // sofort; gemerkt pro Gerät + Profil
    pub fn dispatch(&self, command: Command) -> Result<(), CommandError>; // sofort im Snapshot; Persistenz/Sync im Hintergrund
    pub fn status(&self) -> SyncStatus; // SignedOut (Gast) | Connecting | Live | Offline | Failed { SessionExpired, .. }
    pub fn account(&self) -> Option<Account>;  // None = Gast; { server_url, info: AccountInfo }
    pub fn pending(&self) -> usize;            // unbestätigte Änderungen: vor dem Abmelden warnen
    pub fn set_ai_enabled(&self, enabled: bool) -> Result<AccountInfo, AccountError>; // blockiert
    pub fn sync_until_idle(&self, timeout: Duration) -> SyncOutcome; // blockiert: Idle | Offline | TimedOut
}
```

Außerdem: `dispatch_all` (alles oder nichts), `ai() -> AiClient`, `connections()`, `flush`, `waiter`,
`reconnect_now`. Access-Token für WebSocket und REST; Refresh kurz vor Ablauf und bei 401, serialisiert, neues Paar
(Rotation) wird vor der Benutzung gespeichert; abgelehnter Refresh → `Failed { SessionExpired }`, Daten bleiben,
erneuter Login desselben Kontos → `Renewed`. Der Gast synct nie. Vollständige API: Rustdoc von `quest-store`.

Regel aus Rune: **kein Datei-IO im UI-Thread** – `dispatch` schreibt nur in den Speicher und
übergibt an den Worker-Thread.

**Desktop** (`quest-app`): Tokens im macOS-Schlüsselbund (`KeychainVault("app.quest.desktop")`, `--token-file`
nur für Dev/E2E). Anmelden per Loopback-Redirect: `127.0.0.1:<freier Port>/callback`, Authorize-URL im
System-Browser, danach `finish` + `Profiles::sign_in` im Hintergrund-Thread; anschließend ggf. Übernahme-Dialog für
Gastdaten. Abmelden warnt bei `pending() > 0` und öffnet danach die Logout-Seite des Servers.

## KI-Erfassung (Phase 8)

Freier Text (getippt oder diktiert) wird zu einem **Vorschlag**, den der User prüft und bearbeitet; erst seine
Bestätigung legt etwas an.

1. **Client** baut mit `quest_capture::request` den `CaptureRequest`: Text, Zeitpunkt + IANA-Zeitzone, Sprache und
   einen kompakten Kontext (aktive Quests; Tasks von 7 Tagen vor bis 14 Tage nach heute,
   nächste zuerst, dann offene Tasks ohne Plan-Datum; Limits in `quest-protocol`). Aufruf `POST /v1/ai/capture` über
   `Store::ai()` mit dem bestehenden Bearer-Token.
2. **Server** (`quest-server`, Modul `ai`): Auth, Rate-Limit pro Account, Größenlimits (`check_request`), Prompt +
   JSON-Schema (`prompt`, `output_schema`, strikt für OpenAI-Structured-Outputs) an einen `LlmProvider`
   (`OpenAiCompatible` für OpenRouter und OpenAI; Fallback Tool-Calling; nur an OpenRouter zusätzlich
   `provider.require_parameters`, Fallback-Modelle `models` und `reasoning`). Modellwahl: `docs/ai-models.md`. Schlüssel nur in der Server-Konfiguration;
   ohne Konfiguration antwortet die Route `AiNotConfigured`. Text und Antworten werden nie geloggt.
3. **Validierung** (`quest_capture::validate`): Die Modellantwort ist nicht vertrauenswürdig. Unbekannte Quest-IDs →
   keine Quest, als unsicher markiert; unbekannte Task-IDs → neue Aufgabe (unsicher) oder verworfen; ungültige oder
   weit entfernte Daten, Ende ≤ Start, zukünftige Arbeit, Überlänge und zu viele Elemente werden verworfen und in
   `dropped` gemeldet. Plan-Datum und Deadline laufen durch die Domain-Regel (Plan ≤ Deadline). Neue Entitäten
   tragen temporäre Schlüssel (`q1`, `t1`, `w1`).
4. **Review** (`quest_capture::review::Draft`): Auswahl pro Element, Labels, Fehler pro Element. Eine abgewählte
   neue Quest nimmt sich aus ihren Tasks/Sitzungen heraus.
5. **Anwenden** (`quest_capture::plan`): `CreateQuest`, `CreateTask` mit Plan-Datum und
   Deadline (unten am Tag) + `SetTaskDescription`/`SetTaskRepeat`, `LogWork` (bestehende Aufgabe, auch erledigt,
   oder neue, erledigt angelegte). Fachregeln (Sortierschlüssel, Sitzung zählt zur Quest der Aufgabe, Repeat-Prüfung)
   kommen aus den Commands. Ende `00:00` = Mitternacht am Ende des Tages; Ortszeit → UTC wie überall (früherer
   Zeitpunkt bei doppelter Stunde, übersprungene Zeit eine Stunde später). Angewendet mit `dispatch_all`; Undo
   (`quest_capture::undo`) löscht weich, was noch existiert.

## MCP-Server (Phase 9)

`quest-server` bietet unter `/mcp` einen MCP-Server (rmcp, Streamable HTTP, zustandslos) mit OAuth 2.1 (Server ist
Resource und minimaler Authorization Server; Details in `docs/mcp.md`). Das Modul `mcp` ist nur Adapter: Eingaben
prüfen (`mcp/view.rs`), Commands entscheiden (`mcp/write.rs`, rein), dann `mcp/account.rs`: Felder des Kontos laden →
`Snapshot` → `quest_domain::plan_all` (alles oder nichts) mit einer festen Server-Node-ID pro Konto →
`Repository::apply_changes` → Broadcast an verbundene Geräte, unter derselben Sperre wie Sync-Pushes. Geplante
Wiederholungen werden wie im Plan der Apps über `Command::EditPlan` gespeichert. `capture_text` nutzt dieselbe
KI-Pipeline wie `POST /v1/ai/capture` und dieselbe Review-/Anwenden-Logik (`Draft`, `quest_capture::plan`).

## Anmeldung (Phase 11)

Keine eigenen Passwörter: `quest-server` meldet Nutzer per OpenID Connect beim Identity Provider an (Produktion
ein beliebiger OIDC-Provider, lokal `quest-oidc-fake` oder Keycloak aus `compose.yaml`) und ist selbst Authorization Server für die
Apps (`quest-desktop`, `quest-android`; öffentliche Erstanbieter-Clients mit PKCE, Audience `{origin}/v1`) und für
MCP-Clients (Audience `{origin}/mcp`, Scopes `quest:read`/`quest:write`). Konten entstehen beim ersten Login und
werden über `(issuer, subject)` identifiziert. Der IdP sitzt hinter dem Trait `IdentityProvider` (`oidc.rs`,
openidconnect); Browser-Sitzung, Callback und Logout in `oauth/login.rs`, Entscheidungen (Clients, Ressourcen,
Scopes) pur in `oauth/policy.rs`. KI-Funktionen sind pro Konto opt-in (`users.ai_enabled`). Abläufe, Konfiguration
und Begründungen: `docs/auth.md`.

## Erfolgskriterien

1. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` grün.
2. Server-Integrationstests: OIDC-Login gegen `quest-oidc-fake` (inkl. ungültiger ID-Tokens), Push/Ack, Catch-up ab Cursor, Live-Broadcast an zweite
   Verbindung, LWW-Konflikt (älterer HLC verliert), fremde User sehen nichts.
3. Store-Tests: Offline-Commands persistieren über Neustart; Sync gegen echten In-Process-Server;
   zwei Stores konvergieren nach Offline-Konflikt; Reconnect mit Backoff.
4. App: `cargo run -p quest-app -- --snap <view>.png` rendert jede Ansicht headless;
   ⌘K, Tastenkürzel, Focus-Timer funktionieren.
5. E2E: Server läuft, zwei App-Instanzen mit getrennten Datenverzeichnissen, Änderung in A erscheint
   live in B; A offline geändert → nach Reconnect in B.

## Bewusst offen (Prototyp, vor Produktion nötig)

- Rate-Limits für Anmeldung und OAuth (`/authorize`, `/auth/callback`, `/token`, `/register`) zählen pro
  Client-Adresse in-memory (Details in `docs/auth.md`); hinter einem Reverse Proxy ohne `QUEST_TRUST_FORWARDED_FOR`
  teilen sich alle Clients ein Budget.
- Offene Sync-Verbindungen enden beim Ablauf ihres Access-Tokens und sofort beim Widerruf ihres Grants – aber nur
  für Widerrufe dieser Server-Instanz (In-Process-Broadcast; mehrere Instanzen bräuchten einen gemeinsamen Kanal).
- Konten aus der Passwort-Zeit haben keine Identität; eine Verknüpfung (z. B. per verifizierter E-Mail) gibt es
  nicht automatisch.
- Tokens liegen im Schlüsselbund (macOS) bzw. Keystore (Android), nie in der SQLite; `http://` auch für
  Nicht-Loopback erlaubt.
- Client-SQLite mit `synchronous=NORMAL` (bei Stromausfall können die letzten lokalen Edits fehlen; Konsistenz bleibt).
- Server-Hub räumt verwaiste User-Channels nur lazy auf.
- Kontowechsel per Profil: jedes Konto hat seinen eigenen Store; ein anderes Konto erst nach dem Abmelden.
  Abmelden widerruft das Refresh-Token (best effort) und löscht Tokens und lokale Kontodaten; nicht gesyncte
  Änderungen gehen verloren (die App warnt). Gastdaten werden nur auf Nachfrage übernommen; Gast-Settings nur, wenn
  das Konto vorher aufgeholt hat.
- Refreshes sind nur innerhalb eines Prozesses serialisiert; zwei App-Instanzen auf demselben Konto können eine
  Rotation verlieren (der Server widerruft dann den Grant, erneut anmelden).
- Netzwerk (`quest-store/src/remote.rs`): HTTP und wss prüfen Zertifikate mit `rustls-platform-verifier` (System-Truststore); HTTP-Redirects werden nicht gefolgt, die eingegebene Server-URL muss die endgültige sein.
- Server-Restore (Epoch-Wechsel): Jeder Client lädt nur die Felder erneut hoch, die er selbst geschrieben hat (Server akzeptiert nur `hlc.node == hello.node`). Daten eines Geräts, das nie wieder online kommt, sind nach einem Restore ohne Backup verloren.
