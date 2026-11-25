#!/bin/sh
# Regenerates the PNGs in this folder: every `--scene` of the demo at the reference time, plus
# frames of drags in progress (Rune pointer gestures).
#
# usage: crates/quest-app/snapshots/render.sh [--plan-only]
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target/app}"
app="$CARGO_TARGET_DIR/debug/quest-app"
(cd "$repo" && cargo build -q -p quest-app)

data=$(mktemp -d /tmp/quest-snap.XXXXXX)
trap 'rm -rf "$data"' EXIT
fresh() { rm -rf "$data/q"; echo "$data/q"; }
common="--demo --now 2026-10-15T14:40 --size 1440x900 --scale 2"

if [ "${1:-}" != --plan-only ]; then
for scene in today today-light inbox inbox-light logbook logbook-light tracked-closed tracked-closed-light \
  entry-editor entry-editor-light organizing organizing-light focus quests quests-completed \
  quests-empty-active quests-empty-completed quest-editor quest-new review palette account light \
  quest-detail quest-detail-light quest-colors quest-colors-light quest-rename archive undo-toast task-modal:task-modal-today log-work \
  ai-notice guest-import; do
  # `scene:file` snaps the scene into another file name.
  "$app" $common --data-dir "$(fresh)" --scene "${scene%%:*}" --snap "$here/${scene##*:}.png" >/dev/null 2>&1
done
# The quest picker opens with a short fade: snap it once in place.
for scene in task-modal-quests task-modal-quests-filtered \
  task-detail task-detail-light task-quest-picker task-quest-picker-light \
  task-plan-calendar task-plan-calendar-light task-deadline-calendar task-deadline-calendar-light \
  quick-add-calendar quick-add-calendar-light task-done-toast task-done-toast-light; do
  "$app" $common --data-dir "$(fresh)" --scene "$scene" --snap "$here/$scene.png" --at 0.3 >/dev/null 2>&1
done

fi

for scene in plan light-plan task-modal-plan card-menu repeat-editor shortcuts shortcuts-light; do
  "$app" $common --data-dir "$(fresh)" --scene "$scene" --snap "$here/$scene.png" --at 0.3 >/dev/null
done
e2e="$repo/crates/quest-app/e2e"
snapshot_tree() { python3 -I "$e2e/tree.py" "$data/seed.json" "$@"; }
"$app" $common --data-dir "$data/plan" --scene plan --tree "$data/seed.json" --at 0.2 >/dev/null
task_id=$(snapshot_tree id "Make the first impression feel effortless")
anchor_id=$(snapshot_tree id "Design the next chapter")
quest_id=$(snapshot_tree quest "Write every week")

drag_snapshot() {
  scene=$1 target=$2 name=$3
  sed -e "s/__TASK_ID__/$task_id/" -e "s/__TARGET__/$target/" \
    "$here/scripts/drag-room.json" >"$data/drag.json"
  "$app" $common --data-dir "$data/plan" --scene "$scene" --script "$data/drag.json" \
    --snap "$here/$name.png" --at mark:end >/dev/null
}

for theme in dark light; do
  if [ "$theme" = light ]; then
    plan=light-plan today=today-light suffix=-light
  else
    plan=plan today=today suffix=
  fi
  "$app" $common --data-dir "$data/plan" --scene "$plan" --script "$here/scripts/plan-not-planned.json" \
    --snap "$here/plan-not-planned$suffix.png" --at mark:end >/dev/null
  drag_snapshot "$plan" "plan-head-2026-10-18" "plan-drag$suffix"
  sed "s/__TASK_ID__/$task_id/" "$here/scripts/drag-edge.json" >"$data/edge.json"
  "$app" $common --data-dir "$data/plan" --scene "$plan" --script "$data/edge.json" \
    --snap "$here/plan-drag-edge$suffix.png" --at mark:end >/dev/null
  drag_snapshot "$today" "row-play-$anchor_id" "today-sort-drag$suffix"
  drag_snapshot "$today" "nav-quest-$quest_id" "sidebar-drop$suffix"
done
echo "rendered into $here"
