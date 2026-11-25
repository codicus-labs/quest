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

echo "$summary"
echo "PNGs: $out"
if [ "$failures" -gt 0 ]; then
  echo "E2E FAILED ($failures)"
  exit 1
fi
echo "E2E PASSED"
