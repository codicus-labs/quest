# Rune notes

Notes from building quest-app on Rune `3f0d997`. Each entry gives what happened and the workaround, if one was needed.

## Worked well

- **Setup.** The git dependency plus the copied `[patch.crates-io]` entries built on the first try (about 40 s cold). A hello window and `--snap` worked right away.
- **Headless snapshots.** `--snap`, `--scene`, `--size` and `--scale` make per-view renders trivial. The `scenes` list in `AppSpec` maps directly to "one PNG per view".
- **Theme as data.** `Theme::from_json` with palette, dark and light sections covered the Quest theme (`assets/theme.json`). Light and dark need no code changes.
- **rune-kit.** `rune_kit::root`, `Dialog` and `Button` handle the overlay layer, scrim, dismiss and focus without extra code.
- **Store integration.** `AppHandle::wake()` from the store's worker thread, plus `poll`, is a clean seam. `Tx::wake_after` drives the timer tick without a separate thread.
- **Text features.** `TextStyle::features("'tnum' 1")` gives a stable focus timer.

## Friction and workarounds

- **No command palette in rune-kit.** I built one from `text_input`, a list and `modal` (`src/ui/palette.rs`). It handles Up, Down, Enter and Esc in the field's key handler.
- **No built-in drag and drop between containers.** Rune has `on_drag` but no drop targets, so the app builds cross-container drag and drop itself (see "Drag and drop and the time-tracking calendar" below): cards drag between Plan's day columns, the backlog and Home's list, and onto the time grids.
- **`AppSpec::init` is a fn pointer.** It cannot capture the opened Store, so the app uses `App::init(closure)` instead.
- **First frame needs data.** Headless `--snap` renders the first frame, so the Store is opened in `main` before `run()`, not asynchronously. This is one SQLite open at launch; all later IO runs on the store's worker thread.
- **Deterministic renders need a fixed clock.** The app takes `--now` (via `rune::extra_args()`) and freezes the clock, so snapshots match the reference time (Thu Oct 15, 14:40).
- **Typed letters vs. shortcuts.** A focused text field consumes plain letters before the window's `on_key`, which is the right behaviour. N and F therefore only fire when no field has focus. ⌘K, ⌘1–6 and Esc work everywhere.
- **No hatched or patterned fills.** The reference marks the running block with hatching. Quest uses a translucent fill plus a dashed border.
- **Text overflow in narrow flex rows.** Chips in narrow Plan columns pushed content out of the card. The fix is `min_w(0)` + `shrink` + truncation on the chip, with the move controls on their own row.
- **Text does not wrap or truncate by default.** Without `.wrap()` or `.truncate()`, a text paints past its laid-out rect (`--tree` showed correct rects while the PNG overflowed). Planner titles now use `.wrap()`.
- **Text fields need `.label()`.** Debug builds warn about unnamed fields for screen readers; placeholders do not count.
- **`--tree` helps with layout bugs.** It writes JSON with the rect of every keyed element, which made the overflow above easy to diagnose.
- **Runtime tests.** `Runtime::new` plus `rt.on_key`, `rt.key`, `rt.find` and `pointer_down/up` drive the real view (`src/ui/tests.rs`: ⌘1–6, Esc, ⌘K + Enter, N + typing, play on a card, avatar). One test caught a real bug: the palette ranked "Create task" above a matching view jump.
- **`secure()` / `focus_when_shown(key, ring)`.** These signatures were not obvious from the README; I read them in the source (`crates/rune/src`).

## End-to-end sync (`e2e/run.sh`)

- **`--script` + `--realtime` + `--tree` work well for E2E.** Two headless instances (separate data dirs) drive the account dialog and Home against a real quest-server; assertions read `--tree` JSON (`text`, plus `accessibility` states such as `checked`). It passed 3 out of 3 runs.
- **Scripts address elements by key** (`{"type":"tap","key":"avatar"}`), so they do not break when the layout moves. A missing key aborts the run with a clear message.
- **No variables in scripts.** The random account email and the task id have to be filled into the JSON with `sed` placeholders.
- **No waiting on a condition.** A script cannot wait until text appears; it waits fixed times (`mark end`). That works with a local server but is slower than needed and could become flaky on a slow network.
- **`poll` runs in headless realtime runs**, so the store's background sync shows up without a window. `--realtime` holds each frame until the wall clock catches up.

## Drag and drop and the time-tracking calendar

- **`on_drag` is solid.** It gives pointer capture, `window_point()`, local press points (for the grab offset) and `Phase::Down/Move/Up`. Esc cancels a running drag by itself: `Up` arrives with `cancelled()`. A pure `dnd::step` state machine sits on top without trouble.
- **No drop targets across elements.** Rune has no "what is under the pointer during a drag" for other elements. Workaround: drop zones report their window rects into `State::zones` via `on_layout_tx` + `tx.live_only()` (this also covers scrolling), and `dnd::hit` tests against them in pure code.
- **Scroll coordinate spaces are consistent.** `on_layout` rects and `Drag::window_point()` are both window points after scrolling, so scrolled lists and grids hit-test correctly without any conversion (Runtime tests cover a scrolled plan column and calendar grid). The trap is elsewhere: `on_layout` only fires when the element *moves or resizes*, not when its handler changes. A zone named by its data (`Day(date)`) on an element keyed by position (`plan-col-{i}`) went stale after "Earlier/Later" or a week switch, and drops landed one column off. Rule: name zones like their keys. Plan columns are now keyed by date and their zone is `PlanColumn(list)`; grid days stay positional (`GridDay(grid, i)`) with `drag::geometry` mapping them.
- **Making room animates with plain layout.** Drag moves commit instantly by default; calling `tx.animated()` in the `on_drag` handler (only when the room moves, on drop and on snap back) makes the siblings spring on the layout spring, while the lifted card stays glued to the pointer via `.instant_changes()`. `on_layout` reports where layout puts elements, never spring positions, so hit-testing doesn't see animation. It still sees the open gap, so `dnd::hit` takes it back out (the dragged card's own element, or the ghost's room) and picks the place nearest the lifted card's middle.
- **A keyed element keeps the drag's pointer capture when it moves between siblings.** The dragged card itself moves into its list's room (drawn as the ghost), so no collapse/negative-margin trick is needed. Across lists it stays behind faded and a separate ghost opens the room.
- **Spring back via `matched`.** A flight needs an element that *mounts* in the commit the other leaves; a `.matched` attribute added to an existing element doesn't fly. The card's face therefore changes key on landing (`{prefix}-face-{id}-{n}`) and shares a matched id with the lifted card; `Transition::none()` on the face and lift avoids fades for remounts without a counterpart.
- **`on_press` also fires after a drag** when the release lands inside the source. Grid items therefore handle clicks through the state machine (`Effect::Click`) instead of `on_press`.
- **No patterned fills.** The hatching of tracked sessions is a `canvas` with `Shape::line`s, clipped with `clip_rounded`.
- **No percentage offsets.** Lanes for overlapping items use a row with a `w_frac` spacer. The lane wrappers need stable keys (`{grid}-lane-{id}`); positional ones let items animate from stale places after a delete.
- **Runtime tests need `settle_all` between frames.** `frame(0.0)` doesn't advance springs, so assertions on `presented` rects saw mid-flight values until `settle()` = frame → `settle_all` → frame.
- **Drag scripts are positional.** Taps can target a key, but pointer down/move/up in `--script` take window points. The `snapshots/scripts/dnd-*.json` therefore break when the layout moves.
- **Titlebar.** `App::titlebar(Titlebar::new(56))` with `cx.chrome_insets().leading` as the topbar's left padding keeps the traffic lights off the logo. `--chrome` confirms this headless ("0 overlapping elements").
- **Running sessions** can only have their start dragged; their end is "now", so it has no handle (an app rule, not a Rune limit).

## Round 3: paging, flights, menus, toasts

- **Key scope decides flights.** `.matched(id)` is global: Home's and Plan's card faces shared `card-{id}`, so every Home↔Plan switch flew cards across the screen. Scoping the id per view (`card-{prefix}-{id}`) fixed it; `motion_tests::switching_views_flies_no_card` checks all 30 view pairs and fails with the old id.
- **Paging = keyed columns + asymmetric edges.** Day columns keyed by date (`plan-col-{date}`) keep their identity across a page, so the layout spring moves each column with its cards (children ride the parent). New/old days use `Transition::asymmetric(move_edge(enter), move_edge(leave))` chosen from the paging direction; a `clip_rounded` on the row hides them outside the board. Leaving elements drop out of layout at once (`pop_layout`), so nothing waits for the exit.
- **Menus: rune-kit `Menu`, not `tx.popup_menu`.** The native popup doesn't render in headless PNGs and can't be driven by Runtime tests. Kit `Menu` anchors to the element keyed `{key}-trigger`, so the "…" button swaps its key to `card-menu-trigger` while its menu is open. Items are `{key}-item{i}` (separators count in `i`).
- **Toasts are app-made.** Rune has no toast; a pinned row in the overlay layer plus a pure `UndoState` (expiry passed in, `tx.wake_after` for the timeout) is enough.
- **Removing the focused element moves focus on.** Deleting via a focused button sent focus to the next focusable — a text field — which then swallowed ⌘Z. Every delete path calls `tx.blur()` first.
- **Entering elements take no presses.** At opacity 0 on their first frame they ignore pointer input; Runtime tests must settle before clicking something that just appeared.
- **Card-body drags work with nested buttons.** A press on a child with `on_press` doesn't start the parent's `on_drag`, and the drag starts only past the threshold, so a whole card can be the drag source while its check, chip and "…" stay buttons.

## Round 4: keyboard-first, dialogs, live fields

- **Key routing order decides shortcut conflicts.** With nothing focused the app's `on_key` sees a key first; once an element has focus, Rune's focus handling runs before `on_key`: Tab moves focus and Space presses the focused button. So Tab (cycle views) and Space (start/pause under the pointer) only reach the app while no element is focused; after a click on a button they act on that button. A focused text field takes plain letters before `on_key`, which gives "shortcuts don't fire while typing" for free.
- **Hover as an app state: `on_hover_change` + `tx.live_only()`.** "The task under the cursor" is a `hovered_task` set from each card's `on_hover_change`. `live_only` keeps that handler from counting as a visible change. A card that leaves the list (moved to another day by D/W) reports `inside = false`, so the hover clears by itself. Runtime tests drive it with `pointer_move`.
- **No blur callback: fields save live.** Focus's title and description save in `on_input_change` (one LWW write per edit). There is no "on blur" to save once.
- **Plain Return submits a multi-line field.** A paragraph field calls `on_submit` on Return (or presses the default button); only Shift/Option+Return insert a newline. A notes field that should take Return as a newline needs `on_submit` to `TextEdit::insert("\n")` itself.
- **rune-kit `Dialog` fits form dialogs.** Log work is a `Dialog` (`fit`, `width`, `on_close` for Esc and the close button) with app-made fields; `tx.focus_when_shown(key)` puts the caret in TASK as it opens. A second `on_input_change` on the same field replaces the first, it doesn't add to it.
- **Reading text in tests:** `Runtime::elements()` gives each element's `text`, so tests assert on shown sums (`calendar-quest-hours`, `today-hours`) without a tree dump.

## Round 5: renaming on the card

- **A click on part of a drag source goes through the drag.** A nested `on_press` would stop the card's `on_drag` (see Round 3), so the title stays plain text: it reports its rect via `on_layout_tx` like a drop zone, and `Effect::Click` (a press that didn't pass the threshold) opens the field when the release point lies in it. A drag that starts on the title still moves the card.
- **A text in a `col` stretches to the column's width.** Its laid-out rect (and hit area) covered the whole line, not the words; `.self_start()` (CSS `align-self`) makes it as wide as its text.
- **"Save on blur" via `poll`.** Rune has no per-field blur callback. When focus leaves, the runtime clears `TextEdit::focused` through the field's `on_edit` accessor; the app's `poll` runs once per frame, sees the flag drop and saves. A view switch unmounts the field without that write, so `go()` saves explicitly. Runtime tests must set `rt.poll` themselves.
- **`TextEdit::focused = true` requests focus.** Setting it when the field opens is enough (no `Tx` needed, so it also works from `--scene`); `TextEdit::new` puts the caret at the end.
- **Negative margins work.** `.mx(-7).my(-3)` lets the field's padding and border reach out so its text sits exactly where the title was, without the card changing height.
- *Phase 14 removed the card's title field: a click on the title now opens the task dialog like the rest of the card. The notes above still hold for other inline fields.*

## Round 6: an endless planner, wrapping title fields, a task dialog

- **`scroll_x` gives trackpad scrolling for free.** Native wheel, momentum and axis locking on a horizontal viewport; `tx.scroll_to(key, x, y)` glides, `tx.scroll_to_x` jumps and stops momentum. There is no declarative start offset: the app has to ask for one.
- **A scroll request lands with the commit of the update that asks for it.** Asked from the handler that also mounts the viewport (a tab click, ⌘2), it shows in the first frame. Asked from `poll` or from a `--scene` setup, it lands a frame late, so a `--snap` of the first frame shows offset 0; plan scenes are snapped `--at 0.1`.
- **`on_layout` / `on_layout_tx` report a frame late.** Reading the scroll offset from a content element's rect lags the drawn frame. `on_scroll_offset` reports before each frame draws, but it can't reach the state, so the offset goes through an `Rc<Cell>` the app reads.
- **Virtualise with a leading spacer, never by moving the offset.** Re-setting the offset to compensate for added or removed items would stop momentum. A spacer as wide as the items left out keeps each built item at its fixed position, so the window can change mid-fling without a jump.
- **`settle_all` doesn't advance glides.** Runtime tests drive `scroll_to` glides with real `rt.frame(1/60)` steps; `Cx::layout_size(key)` is right in the first frame already.
- **`wake_after` from `poll` gives a clock for edge auto-scroll.** `poll` runs before the frame's pointer events, so a stop on leaving the edge zone lags by one tick.
- **A multi-line `text_input` wraps and grows; `w_full` ignores negative margins.** With `w_full` the margins move the field but don't widen it, so it wraps narrower than the text it replaces. `self_stretch` lets them widen it. The border is drawn inside the padding, so the margins have to match the padding alone (`-6/-2` for `px(6).py(2)`; the `-7/-3` from Round 5 was 1 pt off).
- **Keeping a field to one line:** swallow Shift/Option-Return in `on_input_key` and handle `on_input_paste` (`ClipboardContent::text()`, `TextEdit::insert`) to replace line breaks. In tests, `tx.request_paste()` works where a scripted ⌘V doesn't reach the field.
- **A click focuses a focusable card, and closing a modal returns focus to it.** Keys on the card's `on_key` would then act on the card clicked last, which amounts to selecting by click. `tx.focus_visible()` tells keyboard focus from pointer focus.
- **`El::modal_tx` makes a custom dialog.** rune-kit's `Dialog` header holds only a title and ×, so a status such as "Saved" needs a custom panel: `modal_tx(click_outside, dismiss)` traps focus and maps Esc and outside clicks to `dismiss`. A filled child covers the panel's border unless the panel is padded by the border width.
- **A blurred field still writes through `on_edit` after the form closed.** The accessor has to return scratch storage (not `get_or_insert`), or the write brings the closed form back.

## Round 7: measuring horizontal scrolling

- **`--native --trace` gives real present times on macOS.** Each frame carries `present_dt_ms`, `target_ms` and `quality.cpu_ms`, and on exit the trace prints p50/p95/max and missed refreshes. It does not record commits. The header's `hz` read 60 on a 120 Hz ProMotion display, although frames were presented every 8.33 ms.
- **`--native --trace` keeps an idle window drawing**: in a debug build, about 100 fps at 100 % CPU with no input, and "idle window still drawing … `wake_after` loop". `sample` puts the time in `record_trace_frame` (the per-frame element snapshot). On a 2500-node view a traced debug frame takes about 70 ms, and input stops arriving, so the app looks hung. Without `--trace` the same binary polls about twice a second and draws nothing. Trace only optimised builds.
- **`--features rune/frame-profile` + `RUNE_FRAME_LOG=FILE`** (headless) writes one JSON line per frame: CPU by phase, `commits`, and `why` (`layout`, `instant`, `dirty`…). It needs no change to the manifest and is the best way to count commits during a scripted scroll. In tests, `rt.stats.commits` counts them.
- **Debug builds add a commit about every 0.25 s while handlers say `live_only`**: the live-only check re-runs the view. A test that counts commits calls `rt.check_live_only(false)`.
- **Trackpad momentum comes from AppKit's momentum phase** (winit maps `momentumPhase`); Rune does not generate it. Synthetic CGEvents need Accessibility permission for the posting terminal; headless `fling` (`vx`) is the replay stand-in.
- **An unoptimised Rune costs about 16–17 ms per frame on a 2500-node view**, even with no commit (release: under 1 ms). `profile.dev.package."*".opt-level = 2` brings the debug build to release-like frame times (p95 3 ms).
- **An `on_layout_tx` that follows a scroll should commit only when what it builds changes.** Comparing a derived value that changes often (here the first visible day) makes it commit on most frames of a fast fling, even though it already says `live_only` otherwise.

## Round 8: editing a time entry

- **`on_press` on a parent does not fire after a drag on a child drag source.** The running session's body keeps `on_press` (it has no body drag) while its start strip drags; a start drag released inside the body opens nothing (Runtime test). The press goes to the drag's own element only.
- **One key per element across the whole tree, also in different columns.** A session over midnight draws a segment in two day columns; keying both by the entry id would collide, so later segments add their day to the key (`{grid}-entry-{id}-{day}`).
- **rune-kit `Button::disabled(bool)`** dims the button and sets `El::disabled` on it (read in the source), so a live validation blocks Save declaratively; ⌘↵ in a field bypasses the button and needs its own guard.
- **A second `on_input_change` replaces the first** (see Round 4) also through helpers: a shared field helper that installs one must take the extra behaviour as a parameter (`changed: fn(&mut State)`) instead of letting callers chain another handler.
- **`--size` per snapshot** shows a whole time grid without scripting a scroll; a scene cannot set a start offset.

## Round 9: idle drawing, hover commits, startup time

- **The live-only check counts as a hover commit.** In debug builds a `live_only` event that comes 0.25 s or more after the last commit still runs the view, to check it. The hover-rows warning counts those runs as commits of the `on_hover_change` handler. A correct live-only hover handler on cards (state the view doesn't read) gets the warning when the pointer crosses 8 cards slowly. Evidence: 7 Home cards crossed 0.3 s apart give 12 commits with the check and 0 without it, or at 120 Hz (`ui/shortcut_tests.rs`). Release builds have 0 commits. The user chose to keep the code (Option B); the warning stays in debug builds.
- **Apps can't turn that check off.** `Runtime::check_live_only(false)` exists only on the headless `Runtime`; `AppSpec`/`App` have no switch for it.
- **`idle window still drawing` comes from a self-renewing `wake_after` whose frames draw nothing new.** Rune counts frames with no input, no motion and the same instance bytes that a due wake or `poll` started; 120 such frames within 2 s give the warning. A held drag sends no input, so an edge scroll that re-arms `wake_after(16 ms)` without moving fits exactly (about 100 frames/s on 120 Hz).
- **The warning names no call site; the frame log does.** With `--features rune/frame-profile` and `RUNE_FRAME_LOG=FILE`, a live window logs per frame `wakers` (each `Tx::wake_after` caller as `file:line duration`, via `#[track_caller]`) and `next` (why the next frame comes). Rune flushes the file after every line, so killing the process loses nothing (docs/perf/idle.md).
- **`Runtime::wake_deadline()` lets a headless test sleep the way a native window does.** Step `rt.frame` to the next deadline and move the app clock with it. Then frames per idle second can be counted; plain `rt.frame(1/60)` loops hide wake loops.
- **Rune's `setup` in "first frame took …" is the process age at `run`**, so it includes `exec`/dyld and macOS's first-run check of a new binary (300–530 ms after each build), not just the code in `main`. Under `cargo run` it also includes cargo's own run, because cargo `exec`s the program in its own process (1043 ms against under 100 ms when started directly; docs/perf/startup.md).
- **`--realtime` headless runs draw every frame at the frame rate** (they don't sleep until wakes), so they can't show idle drawing. Headless frame logs have no `wakers` field.

## Round 10: AI capture (Phase 8)

How it works on the desktop:
- **Entry:** ⌘K → "Capture with AI" (`palette::Action::CaptureWithAi`; hidden when the setting is off).
- **First use:** a privacy notice explains that the text and a short context (active quests, tasks around today) go through the user's Quest server to its AI provider. "Continue" remembers it.
- **Panel:** a multi-line field (Enter adds a line, ⌘↵ or "Structure" sends). While it runs, the panel shows "Structuring…"; Esc or "Stop" goes back to the text and drops the answer. Errors (signed out, offline, not configured, rate limit, …) show the store's `CaptureError` text under the field, and the text stays.
- **Review:** New quests, Tasks and Time entries, each with a checkbox and an editable title. A task's quest is a chip that opens the quest choices (active quests, the proposal's new quests, none); a guessed quest is drawn in the accent colour with "guess". A session on an existing task names it ("Log work on …", "?" when guessed). Left-out items are listed. "Create (n)" is disabled until every selected item is valid (`quest_capture::review::Draft::can_apply`). Esc or the close button keeps the proposal for the next ⌘K (it took a while to get); "Cancel" drops it; "Edit text" goes back to the text. The window's `on_close` cancels a running request.
- **Create** runs `quest_capture::plan` → `Store::dispatch_all` (all or nothing). The toast says "Created 1 quest, 2 tasks and 1 session · Undo ⌘Z". ⌘Z (or Undo) runs `quest_capture::undo` → `dispatch_all`. A later delete takes ⌘Z over; a "Completed" toast only hides the capture toast.
- **Settings:** "AI capture" switch in Account & sync (⌘,). It and the notice flag live in `<data dir>/desktop-settings.json`, read before the event loop and written on a background thread (`src/settings.rs`). Until now the app kept no local settings (the theme is not saved either).
- **Language and zone:** the language comes from `sys-locale` (macOS preferences; a Finder launch has no `LANG`), as a BCP 47 tag, falling back to `en`. The zone is the IANA zone `iana-time-zone` reports, also used for repeats.
- **Test seam:** `--capture-fixture <proposal.json>` answers every capture with that `CaptureProposal`, without network (unit tests set `CaptureSource::Fixture`; the E2E step e uses `e2e/capture-proposal.json`). Snapshots: scenes `capture-notice`, `capture`, `capture-review`, `capture-toast` and `-light` variants.

What Rune gave and where it pinched:
- **`rune::Latest` fits a slow request.** The capture (up to ~75 s) runs on Latest's shared worker; `cancel()` drops the answer for Esc, and the answer lands in `poll`. The window's waker is an `AppHandle::wake` closure set in `App::init`, so headless runs and tests (no waker) still take the answer in `poll`.
- **`on_edit` takes a closure**, so a list of title fields binds each by key (`s.capture.item_title(&key)`), with a scratch `TextEdit` once the proposal is gone.
- **Script modifiers differ by event type.** A `"key"` event takes `"cmd": true`; `"mods": "cmd"` (the form of `"modifier"` events) is ignored silently, so ⌘K and ⌘Z did nothing until the E2E scripts used the boolean.
- **A multi-line `text_input` submits on Enter**; the field inserts the newline itself in `on_submit` and takes ⌘↵ in `on_input_key`.
- **macOS dictation:** not checked by hand in this round (an agent may not send synthetic input, and there was no interactive session). Rune's README says dictation reaches a focused `text_input` as an IME commit (`Ime::Commit`), so Fn Fn / Edit → Start Dictation should type into the capture field. To verify: open the panel in a real window, click the field, press Fn twice, speak, check the words land at the caret. Nothing was rebuilt for it.

## Not verified

- Real window behaviour on screen: only headless snapshots, `--tree` and Runtime tests were checked, not an interactive session.

## Plattform-Reichweite (Lead, 2026-11-22)

- Rune-Roadmap (PLAN.md): macOS + Windows + Linux → web → iOS (M6) → Android (M7). Am gepinnten Commit existiert kein iOS-/Android-Plattformcode.
- Konsequenz: Rune deckt Desktop ab; Mobile braucht vorerst native UI über `quest-store` (z. B. UniFFI) oder Warten auf M6/M7.

## Round 11: sign-in in the browser, profiles, AI opt-in (Phase 11)

How it works on the desktop:
- **Sign in:** Account & sync keeps the server URL; "Sign in" starts a thread that binds `127.0.0.1:0` (`src/loopback.rs`), builds a `LoginRequest` for that port, runs `open <authorize URL>` and waits for the redirect to `/callback`. The page says "Signed in to Quest. You can close this window." "Cancel" sets a flag and connects to the port once to wake the blocking `accept`. Then `LoginRequest::finish` and `Profiles::sign_in` run on the same thread; the result comes back over a channel that `poll` drains (`state/auth.rs`).
- **Profiles:** `main` opens `Profiles` (tokens in the keychain, `KeychainVault`); `State::store` is an `Arc<Store>` so the flows' threads can use it while the UI keeps dispatching. A switch replaces the store and drops the old one on a thread (dropping waits for its worker). Views go back to Home, undo and capture state are cleared.
- **Guest data:** after an account's first sign-in with guest data, "Import guest data into this account?" (Yes → `Profiles::import_guest` on a thread; No/Esc keeps it in the guest store).
- **Sign out:** warns when `Store::pending() > 0` ("Sign out anyway"), then `Profiles::sign_out` and the server's logout page in the browser. `SessionExpired` shows "Sign in again" (same account, `Renewed`).
- **AI features:** an account setting on the server (`set_ai_enabled` on a thread), off by default; turning it on shows the privacy notice (`ui/auth_dialogs.rs`) first. "Capture with AI" in ⌘K shows only with `ai_enabled && ai_available`. The local `desktop-settings.json` and the capture panel's own notice are gone.
- **Dev/E2E seams:** `--token-file <file>` (`FileVault` instead of the keychain) and `--browser <program>` (gets the URL instead of `open`). The E2E run starts its own `quest-oidc-fake` and `quest-server` on free ports; `e2e/browser.py` follows the redirects with a cookie jar and adds `login_hint` at the fake, so the app's real loopback flow runs without a browser.

What Rune gave and where it pinched:
- **Nothing new was needed from Rune.** A channel plus `AppHandle::wake` from the thread fits the same seam as the store's `on_change`; `rune::Latest` would not fit a wait of unknown length (it shares one worker with capture and connections).
- **A newly shown dialog needs `settle_all` in Runtime tests** before its buttons are found (Round 3's "entering elements take no presses" also hides them from `find`).

## Round 12: a quest picker (Phase 20)

How it works: every quest chip (task dialog, Log work, Edit time entry, AI capture, Home's "Add quest work…", Focus) is the trigger of one picker (`ui/quest_picker.rs`, `state/quest_picker.rs`). It opens in the root overlay layer, anchored under the chip by its key, with a search field (focused), the quests most recently used first (`quest_view::quest_picker`), "No quest" last and a max height that scrolls.

What Rune gave and where it pinched:
- **rune-kit's `Combobox` did not fit:** it matches fuzzy subsequences, reorders hits by score and has no leading colour dot. The picker is built from `popover::{anchored, raised, fit_scroll}`, `El::modal(true, …)` (Escape and a press outside close), `focus_return(chip)`, `tx.focus_when_shown`, `tx.scroll_into_view` and `focus_selected` for the highlighted row.
- **`p.selected` is too close to the dark surface** to show the highlighted row; `p.kit.c.hover` (as kit's popup list uses) shows it.
- **A blurred `text_input` may still write once after its popover closed**, so its `on_edit` accessor returns a scratch `TextEdit` when the picker is gone.
- **Anchored overlays stay inside the window only, not inside a dialog.** The picker reads the chip's and the dialog's rects from the frame before (`Cx::window_layout`) and picks its width, side and list height itself (`ui::quest_picker::fit`), so it never runs past the dialog's edge.
