#!/usr/bin/env bash
# Builds Quest.app and a .dmg for this Mac's architecture, ad-hoc signed (no Apple Developer ID):
# macOS asks once on first launch (right-click → Open). Output: target/package/.
set -euo pipefail

repo=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo"

version=$(cargo metadata --no-deps --format-version 1 |
  python3 -I -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "quest-app"))')
arch=$(uname -m)
out="$repo/target/package"
app="$out/Quest.app"
dmg="$out/Quest-$version-macos-$arch.dmg"

cargo build --release --locked -p quest-app

rm -rf "$app" "$dmg"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/quest-app "$app/Contents/MacOS/quest-app"
cp crates/quest-app/assets/Quest.icns "$app/Contents/Resources/Quest.icns"
cat >"$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Quest</string>
  <key>CFBundleDisplayName</key><string>Quest</string>
  <key>CFBundleIdentifier</key><string>app.quest.desktop</string>
  <key>CFBundleExecutable</key><string>quest-app</string>
  <key>CFBundleIconFile</key><string>Quest</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --sign - "$app"

staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
hdiutil create -quiet -volname Quest -srcfolder "$staging" -format UDZO "$dmg"
shasum -a 256 "$dmg" | sed "s|$out/||" >"$dmg.sha256"

echo "$dmg"
