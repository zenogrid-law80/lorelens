#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
bash "$ROOT/packaging/build-macos.sh" release
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$ROOT/dist/LoreLens.app/Contents/Info.plist")"
ARCH="$(uname -m)"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT
ditto "$ROOT/dist/LoreLens.app" "$STAGING/LoreLens.app"
ln -s /Applications "$STAGING/Applications"
DMG="$ROOT/dist/LoreLens-$VERSION-macos-$ARCH.dmg"
hdiutil create -volname "LoreLens $VERSION" -srcfolder "$STAGING" -ov -format UDZO "$DMG"
hdiutil verify "$DMG"
echo "Built $DMG"
