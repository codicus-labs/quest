#!/bin/sh
# End-to-end sync through the real UI: headless Quest instances (one data dir each) drive the
# account dialog and Today with Rune `--script --realtime` and assert on `--tree` JSON.
#
# The run starts its own OIDC fake (quest-oidc-fake, users anna and ben) and its own quest-server
# on free loopback ports, and stops both at the end. Signing in goes through the app's real flow
# (loopback listener, code exchange, profile switch); only the browser is replaced by
# e2e/browser.py (`--browser`), which follows the redirects and picks the user at the fake with
# login_hint=$QUEST_E2E_USER. Tokens go to a file per instance (`--token-file`), not the keychain.
#
# r2c  Click-to-open, persisted Today ordering, moving between days and filing in a quest.
# a–d  A signs in (anna) and adds a task, B (anna) receives it, completes it, and gets a task live.
# e    AI: ben (AI features off) gets no "Capture with AI" in ⌘K; anna turns AI features on (privacy
#      notice), then captures (proposal from a fixture, `--capture-fixture`) and takes it back (⌘Z).
# f    Guest data: a guest task, then ben signs in and imports it; signing out and in as anna shows
#      only anna's data.
# w    Workspaces: A creates "Work" with a task; a restart keeps it shown; B (same account) sees the
#      task only after switching to Work; A renames it, deletes it (B falls back to Default) and
#      restores it.
#
# usage: crates/quest-app/e2e/run.sh
# env:   QUEST_E2E_DIR  default /tmp/quest-e2e (wiped; nothing else is touched)
set -u

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
root=${QUEST_E2E_DIR:-/tmp/quest-e2e}
out="$here/out"
work="$root/scripts"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target/app}"
bin="$CARGO_TARGET_DIR/debug"
app="$bin/quest-app"

failures=0
summary=""

pass() { summary="$summary
PASS  $1"; }
fail() { summary="$summary
FAIL  $1"; failures=$((failures + 1)); }

free_port() { python3 -I -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'; }

# Waits until URL answers (the fake and the server start in under a second).
await_url() {
  i=0
  until curl -s -o /dev/null --max-time 1 "$1"; do
    i=$((i + 1))
    [ "$i" -gt 100 ] && return 1
    sleep 0.1
  done
}

# A script template with this run's server (and task id) filled in.
script() {
  sed -e "s|__SERVER__|$server|" -e "s/__TASK_ID__/${task_id:-}/" -e "s/__WORKSPACE__/${workspace_id:-}/" \
    -e "s/__DAY__/${day:-}/" -e "s/__QUEST_ID__/${quest_id:-}/" -e "s/__RENAMED__/${renamed:-}/" \
    "$here/$1.json" >"$work/$1.json"
  echo "$work/$1.json"
}

# run USER DIR SCRIPT NAME [FLAGS…]: one realtime headless run as USER (who signs in at the fake)
# that leaves NAME.png and NAME.json in $out.
run() {
  user=$1 dir=$2 template=$3 name=$4
  shift 4
  QUEST_E2E_USER=$user QUEST_E2E_IDP=$idp "$app" --data-dir "$dir" --token-file "$dir/tokens.json" \
    --browser "$here/browser.py" "$@" --script "$(script "$template")" --realtime --size 1440x900 --scale 2 \
    --snap "$out/$name.png" --tree "$out/$name.json" >"$out/$name.log" 2>&1
}

tree() { python3 -I "$here/tree.py" "$@"; }

# check NAME DESCRIPTION QUESTION ARG [ARG2]: one assertion on NAME's tree.
check() {
  if tree "$out/$1.json" "$3" "$4" ${5:+"$5"}; then pass "$2"; else fail "$2 (see $out/$1.log)"; fi
}

# absent NAME DESCRIPTION TITLE: NAME's Today shows no task row TITLE.
absent() {
  if tree "$out/$1.json" has "$3"; then fail "$2 (see $out/$1.log)"; else pass "$2"; fi
}

(cd "$repo" && cargo build -q -p quest-app -p quest-server -p quest-oidc-fake) || { echo "FAIL  build" >&2; exit 1; }
rm -rf "$root" "$out"
mkdir -p "$root" "$work" "$out"

idp_port=$(free_port)
server_port=$(free_port)
idp="http://127.0.0.1:$idp_port"
server="http://127.0.0.1:$server_port"
"$bin/quest-oidc-fake" --bind "127.0.0.1:$idp_port" >"$out/oidc-fake.log" 2>&1 &
idp_pid=$!
QUEST_BIND="127.0.0.1:$server_port" QUEST_DATABASE_URL="sqlite://$root/server.sqlite" \
  QUEST_OIDC_ISSUER="$idp" QUEST_OIDC_CLIENT_ID=quest-server QUEST_OIDC_CLIENT_SECRET=quest-dev-secret \
  QUEST_AI_BASE_URL=http://127.0.0.1:9/v1 QUEST_AI_API_KEY=e2e QUEST_AI_MODEL=e2e \
  "$bin/quest-server" >"$out/quest-server.log" 2>&1 &
server_pid=$!
trap 'kill "$idp_pid" "$server_pid" 2>/dev/null' EXIT INT TERM
if ! await_url "$idp/.well-known/openid-configuration" || ! await_url "$server/"; then
  echo "FAIL  the OIDC fake or quest-server did not start (see $out/oidc-fake.log, $out/quest-server.log)" >&2
  exit 1
fi
echo "server: $server  identity provider: $idp"

run anna "$root/r3" r3-add r3-add --demo --now 2026-11-24T13:00
check r3-add "r3 Quick Add planned the task for Today" has "R3 draft"
task_id=$(tree "$out/r3-add.json" id "R3 draft" 2>/dev/null)
if [ -n "$task_id" ]; then
  check r3-add "r3 Quick Add assigned the quest" rowtext "R3 draft" "Run a half marathon"
  run anna "$root/r3" r3-undo r3-undo --now 2026-11-24T13:00
  check r3-undo "r3 Undo reopened the completed task" open "R3 draft"
else
  fail "r3 skipped: no task for Undo"
fi

r2c_time=2026-11-24T13:00
moving="Make the first impression feel effortless"
anchor="Design the next chapter"
quest="Write every week"
day=2026-11-28
run anna "$root/r2c" r2c-inspect r2c-seed --demo --now "$r2c_time" --scene today
task_id=$(tree "$out/r2c-seed.json" id "$moving" 2>/dev/null)
quest_id=$(tree "$out/r2c-seed.json" quest "$quest" 2>/dev/null)
if [ -n "$task_id" ] && [ -n "$quest_id" ]; then
  run anna "$root/r2c" r2c-detail r2c-detail --now "$r2c_time" --scene today
  check r2c-detail "r2c one click opens the inline detail" key task-modal-title
  run anna "$root/r2c" r2c-reorder r2c-reordered --now "$r2c_time" --scene today
  check r2c-reordered "r2c Today drag reorders the row" above "$moving" "$anchor"
  run anna "$root/r2c" r2c-inspect r2c-restart --now "$r2c_time" --scene today
  check r2c-restart "r2c Today order persists after restart" above "$moving" "$anchor"
  run anna "$root/r2c" r2c-replan r2c-replanned --now "$r2c_time" --scene plan
  check r2c-replanned "r2c Plan drag changes the plan date" under "$moving" "$day"
  run anna "$root/r2c" r2c-file r2c-filed --now "$r2c_time" --scene plan
  check r2c-filed "r2c sidebar drag assigns the quest" rowtext "$moving" "$quest"

  task_id=$(tree "$out/r2c-seed.json" id "$anchor" 2>/dev/null)
  renamed="R2d title edited in place"
  run anna "$root/r2c" r2d-title r2d-title --now "$r2c_time" --scene today
  check r2d-title "r2d title saves in the collapsed row after Esc" keytext "row-title-$task_id" "$renamed"

  day=2026-12-04
  run anna "$root/r2c" r2c-inspect r2d-plan-before --now "$r2c_time" --scene plan
  check r2d-plan-before "r2d target day starts below the viewport" below "plan-head-$day" plan-page-scroll
  run anna "$root/r2c" r2d-scroll r2d-plan-after --now "$r2c_time" --scene plan
  check r2d-plan-after "r2d drag auto-scrolls to a previously offscreen day" under "$renamed" "$day"
else
  fail "r2c skipped: demo task or quest not found"
fi

run anna "$root/r4" r2c-inspect r4-seed --demo --now "$r2c_time" --scene today
task_id=$(tree "$out/r4-seed.json" id "$anchor" 2>/dev/null)
if [ -n "$task_id" ]; then
  run anna "$root/r4" r4-detail r4-detail --now "$r2c_time" --scene today
  check r4-detail "r4 Notes edits Markdown and renders it after Esc" keytext note-block-0 "R4 rendered note"
  check r4-detail "r4 Enter in an item creates a new editable item" shows "Follow up"
  run anna "$root/r4" r2c-inspect r4-list --now "$r2c_time" --scene today
  check r4-list "r4 two items checked with Cmd+Enter, row shows 2/5 after restart" keytext "row-checklist-$task_id" "2/5"
else
  fail "r4 skipped: demo task not found"
fi

# a. A signs in as anna through the browser and adds a task on Today.
run anna "$root/a" a-sign-in a-signed-in
check a-signed-in "a  A signed in as anna" shows "Signed in as Anna Example"
run anna "$root/a" a-create a-after-create
check a-after-create "a  A created 'E2E from A'" has "E2E from A"

# b. B signs in as anna too and receives it.
run anna "$root/b" b-sign-in b-after-sign-in
check b-after-sign-in "b  B received 'E2E from A'" has "E2E from A"
task_id=$(tree "$out/b-after-sign-in.json" id "E2E from A" 2>/dev/null)

# c. B completes it; A, started again later, shows it done.
if [ -n "$task_id" ]; then
  run anna "$root/b" b-complete b-after-complete
  check b-after-complete "c  B completed it" done "E2E from A"
  run anna "$root/a" a-wait a-after-restart
  check a-after-restart "c  A shows it done after a restart" done "E2E from A"
else
  fail "c  skipped: B has no 'E2E from A' to complete"
fi

# d. A keeps running while B creates a task; A gets it live.
run anna "$root/a" a-live a-live &
a_pid=$!
sleep 4
run anna "$root/b" b-create b-after-create
wait "$a_pid"
check b-after-create "d  B created 'Live from B'" has "Live from B"
check a-live "d  running A received 'Live from B' live" has "Live from B"

# e. AI features: off by default, on after the privacy notice; then capture and ⌘Z.
sed -e "s/__TODAY__/$(date +%F)/" -e "s/__YESTERDAY__/$(date -v-1d +%F)/" "$here/capture-proposal.json" \
  >"$work/capture-proposal.json"
fixture="--capture-fixture $work/capture-proposal.json"
run ben "$root/e0" e-no-ai e-no-ai
if tree "$out/e-no-ai.json" key palette-input && ! tree "$out/e-no-ai.json" shows "Capture with AI"; then
  pass "e  AI features off: ⌘K offers no 'Capture with AI'"
else
  fail "e  AI features off: ⌘K offers no 'Capture with AI' (see $out/e-no-ai.log)"
fi
# shellcheck disable=SC2086
run anna "$root/c" e-ai-notice e-ai-notice $fixture
check e-ai-notice "e  turning AI features on shows the privacy notice" key ai-notice-text
# shellcheck disable=SC2086
run anna "$root/c" e-ai-on e-ai-on $fixture
check e-ai-on "e  AI features are on after 'Turn on'" checked account-ai-features
# shellcheck disable=SC2086
run anna "$root/c" c-capture c-after-capture $fixture
check c-after-capture "e  capture created 'E2E captured task'" has "E2E captured task"
# shellcheck disable=SC2086
run anna "$root/c" c-capture-undo c-after-undo $fixture
# After ⌘Z the script opens the archive: the captured task is there (created, then deleted by the undo).
if [ ! -s "$out/c-after-undo.json" ] || grep -q "no element" "$out/c-after-undo.log"; then
  fail "e  capture + ⌘Z run (see $out/c-after-undo.log)"
elif grep -q '"archive-item-' "$out/c-after-undo.json" && grep -q "E2E captured task" "$out/c-after-undo.json"; then
  pass "e  ⌘Z took the capture back (it is in the archive)"
else
  fail "e  ⌘Z took the capture back (archive shows 'E2E captured task')"
fi

# f. Guest data: imported into ben's account on his first sign-in; switching to anna shows only hers.
run ben "$root/g2" g-guest g2-guest
run ben "$root/g2" g-import-offer guest-import-offer
check guest-import-offer "f  the first sign-in offers to import the guest data" key guest-import-yes
run ben "$root/g" g-guest g-guest
check g-guest "f  the guest created 'Guest task'" has "Guest task"
run ben "$root/g" g-import g-imported
check g-imported "f  ben's account has the imported 'Guest task'" has "Guest task"
run anna "$root/g" g-switch g-switched
check g-switched "f  after signing out and in as anna: anna's 'E2E from A'" has "E2E from A"
if tree "$out/g-switched.json" has "Guest task"; then
  fail "f  anna doesn't see ben's 'Guest task'"
else
  pass "f  anna doesn't see ben's 'Guest task'"
fi

# w. Workspaces, in one account on two instances.
run anna "$root/a" w-a-create w-a-created
check w-a-created "w  A created workspace 'Work' and shows it" keytext workspace-name Work
check w-a-created "w  A added 'Work task' in Work" has "Work task"
absent w-a-created "w  Work doesn't show Default's 'E2E from A'" "E2E from A"
run anna "$root/a" w-a-manage w-a-restarted
check w-a-restarted "w  a restart keeps Work shown" keytext workspace-name Work
workspace_id=$(tree "$out/w-a-restarted.json" workspace Work 2>/dev/null)
run anna "$root/b" w-b-default w-b-default
check w-b-default "w  B still shows Default's 'E2E from A'" has "E2E from A"
absent w-b-default "w  B's Default doesn't show 'Work task'" "Work task"
run anna "$root/b" w-b-switch w-b-switched
check w-b-switched "w  B switched to Work and has 'Work task'" has "Work task"
absent w-b-switched "w  B's Work doesn't show 'E2E from A'" "E2E from A"
if [ -n "$workspace_id" ]; then
  run anna "$root/a" w-a-rename w-a-renamed
  check w-a-renamed "w  A renamed Work to Office" keytext workspace-name Office
  run anna "$root/a" w-a-delete w-a-deleted
  check w-a-deleted "w  deleting the shown workspace shows Default" keytext workspace-name Default
  check w-a-deleted "w  the deleted workspace waits under Deleted" key "workspace-restore-$workspace_id"
  run anna "$root/b" w-b-default w-b-after-delete
  check w-b-after-delete "w  B falls back to Default after the delete" keytext workspace-name Default
  absent w-b-after-delete "w  B no longer shows 'Work task'" "Work task"
  run anna "$root/a" w-a-restore w-a-restored
  check w-a-restored "w  A restored Office and shows 'Work task'" has "Work task"
  check w-a-restored "w  the restored workspace keeps its new name" keytext workspace-name Office
else
  fail "w  skipped: no id for workspace 'Work'"
fi

# R5 replays the same route in fresh profiles and inspects each checkpoint.
for stage in started organizing returned hint booked undone; do
  run anna "$root/r5-$stage" r5-focus "r5-$stage" --demo --now 2026-11-25T14:40 --at "mark:$stage" \
    || fail "r5 $stage script (see $out/r5-$stage.log)"
done
check r5-started "r5 starting a task opens Focus" key focus-timer
check r5-organizing "r5 O switches to Organizing" key player-back-to
check r5-returned "r5 Back to Task opens Focus again" key focus-timer
check r5-hint "r5 two minutes outside Focus show Book" key organizing-book
check r5-hint "r5 outside part of the task run is hatched" key timeline-outside-focus
check r5-booked "r5 Booking splits task and Organizing at 14:44" split "Design the next chapter"
check r5-undone "r5 Undo rejoins the running task entry" joined "Design the next chapter"

for stage in current previous picker tomorrow detail deleted restored; do
  run anna "$root/r5b-$stage" r5b-review "r5b-$stage" --demo --now 2026-11-27T14:40 --scene review-demo-seed --at "mark:$stage" \
    || fail "r5b $stage script (see $out/r5b-$stage.log)"
done
review_count=review-count-00000000-0000-4000-8000-000000000005
check r5b-current "r5b Cmd+5 shows Review" key review
check r5b-current "r5b current week starts on Monday" keytext review-range "Nov 23 – 29"
check r5b-previous "r5b [ shows the previous week" keytext review-range "Nov 16 – 22"
check r5b-previous "r5b the right column still shows This week" key this-week
check r5b-current "r5b the task started with four postponements" keytext "$review_count" "↷ 4"
check r5b-picker "r5b Schedule opens the date picker" key task-calendar
check r5b-tomorrow "r5b Tomorrow adds a postponement" keytext "$review_count" "↷ 5"
check r5b-detail "r5b a title opens the standard task detail" key task-modal-title
check r5b-deleted "r5b Delete removes the task from the ranking" no-live-key "$review_count"
check r5b-deleted "r5b Delete offers Undo" key toast-action
check r5b-restored "r5b Undo restores the task and its history" keytext "$review_count" "↷ 5"

for stage in before running returned after; do
  run anna "$root/r5b-snippet-$stage" r5b-snippet "r5b-snippet-$stage" --demo --at "mark:$stage" \
    || fail "r5b snippet $stage script (see $out/r5b-snippet-$stage.log)"
done
check r5b-snippet-running "r5b Organize starts a session" key player-back-to
check r5b-snippet-returned "r5b immediate Back to Task resumes Focus" key focus-timer
before=$(tree "$out/r5b-snippet-before.json" organizing-count _)
running=$(tree "$out/r5b-snippet-running.json" organizing-count _)
after=$(tree "$out/r5b-snippet-after.json" organizing-count _)
if [ -n "$before" ] && [ "$running" = "$((before + 1))" ] && [ "$before" = "$after" ]; then
  pass "r5b Back to Task removes the short Organizing block"
else
  fail "r5b Organizing blocks before/running/after: $before/$running/$after"
fi

for stage in panel calendar picker-closed panel-closed hotkey-closed focus-closed saved; do
  run anna "$root/r6-panel" r6-panel "r6-panel-$stage" --window quick-add --now "$r2c_time" --at "mark:$stage" \
    || fail "r6 quick-add $stage script (see $out/r6-panel-$stage.log)"
done
check r6-panel-panel "r6 virtual global shortcut focuses Quick Add" focused quick-add
check r6-panel-calendar "r6 Cmd+Shift+P opens the shared calendar" key task-calendar
check r6-panel-picker-closed "r6 Escape closes only the picker" no-live-key task-calendar
check r6-panel-picker-closed "r6 Escape restores text focus" focused quick-add
for stage in panel-closed hotkey-closed focus-closed saved; do
  check "r6-panel-$stage" "r6 panel closes: $stage" no-live-key quick-add
done
check r6-panel-saved "r6 feedback says Today" shows Today
run anna "$root/r6-panel" r2c-inspect r6-today --now "$r2c_time" --scene today
check r6-today "r6 panel task persisted to Today" has "Book dentist"

for stage in running paused resumed stopped; do
  run anna "$root/r6-widget-$stage" r6-widget "r6-widget-$stage" --window timer-widget --demo --now "$r2c_time" --at "mark:$stage" \
    || fail "r6 widget $stage script (see $out/r6-widget-$stage.log)"
done
check r6-widget-running "r6 running task can pause" key widget-pause
check r6-widget-paused "r6 pause keeps widget with Resume" key widget-resume
elapsed=$(tree "$out/r6-widget-running.json" text-of widget-time)
check r6-widget-paused "r6 paused time remains frozen" keytext widget-time "$elapsed"
check r6-widget-resumed "r6 Resume starts a new entry" keytext widget-time "00:00"
check r6-widget-resumed "r6 Resume starts the same task" shows "Design the next chapter"
check r6-widget-stopped "r6 Stop clears widget" no-live-key timer-widget

for stage in organizing back; do
  dir="$root/r6-organizing-$stage"
  run anna "$dir" r2c-inspect "r6-seed-$stage" --demo --now "$r2c_time" --scene organizing \
    || fail "r6 Organizing seed (see $out/r6-seed-$stage.log)"
  run anna "$dir" r6-widget-back "r6-$stage" --window timer-widget --now "$r2c_time" --at "mark:$stage" \
    || fail "r6 organizing $stage script (see $out/r6-$stage.log)"
done
check r6-organizing "r6 Organizing offers Back to Task" key widget-back
check r6-back "r6 Back to Task runs the original task" shows "Design the next chapter"
check r6-back "r6 Back to Task removes the Back button" no-live-key widget-back

r7_time=2026-11-25T14:40
for stage in quest calendar saved; do
  run anna "$root/r7-plan-$stage" r7-plan-add "r7-plan-$stage" --demo --now "$r7_time" --at "mark:$stage" \
    || fail "r7 Plan quick-add $stage"
done
check r7-plan-quest "r7 Plan N then Cmd+Shift+L focuses Quest" focused quest-picker-search
check r7-plan-calendar "r7 Plan quick-add shows the keyboard calendar" key calendar-day-2026-12-04
check r7-plan-saved "r7 l l j Enter places the task on December 4" under "R7 planned task" 2026-12-04

run anna "$root/r7-focus" r7-focus r7-focus --demo --now "$r7_time" --at mark:renamed
check r7-focus "r7 editing and Esc leave the timer in Focus" key focus-timer
check r7-focus "r7 checklist item renamed in Focus" shows "X O B final"
run anna "$root/r7-focus" r2c-inspect r7-focus-restart --now "$r7_time" --scene focus
check r7-focus-restart "r7 renamed checklist item survived restart" shows "X O B final"

for stage in repeat deleted restored; do
  dir="$root/r7-detail-$stage"
  run anna "$dir" r2c-inspect "r7-seed-$stage" --demo --now "$r7_time" --scene home
  task_id=$(tree "$out/r7-seed-$stage.json" id "$anchor" 2>/dev/null)
  run anna "$dir" r7-detail "r7-detail-$stage" --now "$r7_time" --scene home --at "mark:$stage" \
    || fail "r7 detail $stage"
done
check r7-detail-repeat "r7 R opens the repeat editor" key repeat-editor-dialog
absent r7-detail-deleted "r7 Cmd+Backspace in detail deletes the task" "$anchor"
check r7-detail-deleted "r7 deleting offers Undo" key toast-action
check r7-detail-restored "r7 Cmd+Z restores deleted task" has "$anchor"

for stage in quest-first quest plan-first plan deadline-first deadline date-applied today note-escaped saved; do
  dir="$root/r7-panel-$stage"
  run anna "$dir" r7-panel "r7-panel-$stage" --window quick-add --demo --now "$r7_time" --at "mark:$stage" \
    || fail "r7 panel $stage"
done
check r7-panel-quest "r7 panel Cmd+Shift+L focuses Quest" focused quest-picker-search
check r7-panel-plan "r7 panel Cmd+Shift+P focuses the Plan calendar" focused task-calendar
run anna "$root/r7-panel-menu" r7-panel-menu r7-panel-menu --window quick-add --demo --now "$r7_time" \
  --desktop "$out/r7-seed-restored.png" --at mark:end
check r7-panel-menu "r7 the main menu forwards Quest to the key panel" focused quest-picker-search
check r7-panel-deadline "r7 panel Cmd+Shift+D focuses the Deadline calendar" focused task-calendar
for stage in quest plan deadline; do
  key=task-calendar
  [ "$stage" != quest ] || key=quest-picker
  check "r7-panel-$stage-first" "r7 $stage picker is full-size and below the title on its first frame" \
    stable-picker "$key" "$out/r7-panel-$stage.json"
done
check r7-panel-date-applied "r7 Vim navigation applies a date in the panel" shows "Fri 4 Dec"
check r7-panel-today "r7 T applies a quick date in the panel" no-live-key task-calendar
check r7-panel-note-escaped "r7 note Esc returns focus to the panel title" focused quick-add
check r7-panel-saved "r7 note Cmd+Enter submits the panel" key quick-add-feedback
run anna "$root/r7-panel-saved" r2c-inspect r7-note-seed --now "$r7_time" --scene home
task_id=$(tree "$out/r7-note-seed.json" id "R7 panel note" 2>/dev/null)
if [ -n "$task_id" ]; then
  run anna "$root/r7-panel-saved" r7-open r7-note-open --now "$r7_time" --scene home
  check r7-note-open "r7 the Markdown source and its newline survived restart" \
    keytext task-modal-description-source "$(printf '## R7 saved Markdown\nSecond line')"
else
  fail "r7 panel task with note not found after restart"
fi

echo "$summary"
echo "PNGs: $out"
if [ "$failures" -gt 0 ]; then
  echo "E2E FAILED ($failures)"
  exit 1
fi
echo "E2E PASSED"
