#!/usr/bin/env bash
# Renders icon.svg into Quest.icns (macOS tools only: qlmanage, sips, iconutil). Run after changing icon.svg.
set -euo pipefail

assets=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

qlmanage -t -s 1024 -o "$work" "$assets/icon.svg" >/dev/null
master="$work/icon.svg.png"
iconset="$work/Quest.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$master" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$master" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$assets/Quest.icns"
