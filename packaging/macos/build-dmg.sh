#!/usr/bin/env bash
# Builds "Ember IDE.app" and a .dmg. Run on macOS from the repo root after `cargo build --release`.
# Usage: packaging/macos/build-dmg.sh <version> [target-dir]
set -euo pipefail
VERSION="${1:-0.1.0}"
BIN="${2:-target/release}/ember"
APP="dist/Ember IDE.app"

rm -rf "$APP" dist/icon.iconset
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" dist/icon.iconset

cp "$BIN" "$APP/Contents/MacOS/ember"
cp -R plugins "$APP/Contents/Resources/plugins"
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

# icns from the 512px png
for s in 16 32 64 128 256 512; do
  sips -z $s $s assets/icon.png --out "dist/icon.iconset/icon_${s}x${s}.png" >/dev/null
done
cp dist/icon.iconset/icon_32x32.png   dist/icon.iconset/icon_16x16@2x.png
cp dist/icon.iconset/icon_64x64.png   dist/icon.iconset/icon_32x32@2x.png
cp dist/icon.iconset/icon_256x256.png dist/icon.iconset/icon_128x128@2x.png
cp dist/icon.iconset/icon_512x512.png dist/icon.iconset/icon_256x256@2x.png
iconutil -c icns dist/icon.iconset -o "$APP/Contents/Resources/icon.icns"
rm -rf dist/icon.iconset

# ad-hoc sign so Apple Silicon will launch it (users right-click > Open the first time)
codesign --force --deep --sign - "$APP" || true

hdiutil create -volname "Ember IDE" -srcfolder "$APP" -ov -format UDZO "dist/Ember-IDE-$VERSION.dmg"
echo "Built dist/Ember-IDE-$VERSION.dmg"
