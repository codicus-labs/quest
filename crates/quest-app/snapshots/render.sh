#!/bin/sh
# Regenerates the PNGs in this folder: every `--scene` of the demo at the reference time, plus
# frames of drags in progress (`scripts/dnd-*.json`, pointer gestures by window point).
#
# usage: crates/quest-app/snapshots/render.sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target/app}"
app="$CARGO_TARGET_DIR/debug/quest-app"
(cd "$repo" && cargo build -q -p quest-app)

data=$(mktemp -d)
trap 'rm -rf "$data"' EXIT
fresh() { rm -rf "$data/q"; echo "$data/q"; }
common="--demo --now 2026-10-15T14:40 --size 1100x750 --scale 2"

for scene in home calendar focus quests quests-completed quests-empty-active quests-empty-completed quest-editor \
  quest-new review palette account light home-tracked \
  quest-detail quest-detail-no-milestones quest-rename archive undo-toast card-menu task-modal:task-modal-home log-work shortcuts \
  entry-editor ai-notice guest-import; do
  # `scene:file` snaps the scene into another file name.
  "$app" $common --data-dir "$(fresh)" --scene "${scene%%:*}" --snap "$here/${scene##*:}.png" >/dev/null 2>&1
done
# A session moved over midnight: tall enough to show the grid from 07:00 to 23:00.
"$app" --demo --now 2026-10-15T14:40 --size 1100x1020 --scale 2 --data-dir "$(fresh)" --scene entry-midnight \
  --snap "$here/entry-midnight.png" >/dev/null 2>&1
# The planner puts its strip in place a frame after a scene mounts it: snap a moment later.
for scene in plan light-plan task-modal-plan repeat-editor; do
  "$app" $common --data-dir "$(fresh)" --scene "${scene%%:*}" --snap "$here/${scene##*:}.png" --at 0.1 >/dev/null 2>&1
done

# The quest picker opens with a short fade: snap it once in place.
for scene in task-modal-quests task-modal-quests-filtered; do
  "$app" $common --data-dir "$(fresh)" --scene "$scene" --snap "$here/$scene.png" --at 0.3 >/dev/null 2>&1
done

for script in dnd-list dnd-grid dnd-new-session; do
  "$app" $common --data-dir "$(fresh)" --scene home --script "$here/scripts/$script.json" \
    --snap "$here/$script.png" --at mark:end >/dev/null 2>&1
done
"$app" $common --data-dir "$(fresh)" --scene calendar --script "$here/scripts/dnd-calendar.json" \
  --snap "$here/dnd-calendar.png" --at mark:end >/dev/null 2>&1
"$app" $common --data-dir "$(fresh)" --scene plan --script "$here/scripts/dnd-plan-scrolled.json" \
  --snap "$here/dnd-plan-scrolled.png" --at mark:end >/dev/null 2>&1
# A trackpad swipe a few weeks on: the month follows what shows.
"$app" $common --data-dir "$(fresh)" --scene plan --script "$here/scripts/plan-scroll.json" \
  --snap "$here/plan-scroll.png" --at mark:end >/dev/null 2>&1
# The room opening in Home's list as the card is dragged up: one crop of the list per frame.
"$app" $common --data-dir "$(fresh)" --scene home --script "$here/scripts/dnd-list.json" \
  --start 0.3 --frames 36 --every 3 --cols 4 --crop 24,190,556,520 \
  --strip "$here/dnd-make-room-strip.png" >/dev/null 2>&1
# Later glides the planner one day on: the columns move as one board, with their cards.
"$app" $common --data-dir "$(fresh)" --scene plan --script "$here/scripts/plan-paging.json" \
  --start 0.3 --frames 8 --every 3 --cols 2 --crop 0,150,1100,250 \
  --strip "$here/plan-paging-strip.png" >/dev/null 2>&1
echo "rendered into $here"
