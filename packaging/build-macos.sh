#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILE="${1:-release}"
case "$PROFILE" in
    release) BUILD_ARGS=(--release) ;;
    debug) BUILD_ARGS=() ;;
    *) echo "Usage: bash packaging/build-macos.sh [release|debug]" >&2; exit 1 ;;
esac
if [[ "$(uname -s)" != Darwin ]]; then
    echo "This script requires macOS and Xcode tools." >&2
    exit 1
fi

cd "$ROOT"
cargo build -p lorelens ${BUILD_ARGS[@]+"${BUILD_ARGS[@]}"}
# Ask Cargo for the output directory so CARGO_TARGET_DIR is respected.
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 | /usr/bin/plutil -extract target_directory raw -o - -)"
APP="$ROOT/dist/LoreLens.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET_DIR/$PROFILE/lorelens" "$APP/Contents/MacOS/LoreLens"
# Match the bundled CLI path resolved relative to the app executable when available.
if [[ -f "$ROOT/lorelens/dist/lore" ]]; then
    mkdir -p "$APP/Contents/MacOS/dis"
    cp "$ROOT/lorelens/dist/lore" "$APP/Contents/MacOS/dis/lore"
    chmod +x "$APP/Contents/MacOS/dis/lore"
else
    # Do not carry a CLI from an earlier build into this app.
    rm -f "$APP/Contents/MacOS/dis/lore"
    echo "Building without a bundled Lore CLI; use Locate CLI or lore on PATH." >&2
fi

ICON_TMP="$(mktemp -d)"
trap 'rm -rf "$ICON_TMP"' EXIT
ICONSET="$ICON_TMP/LoreLens.iconset"
mkdir -p "$ICONSET"
sips -s format png lorelens/dist/favicon.ico --out "$ICON_TMP/icon.png" >/dev/null
for SIZE in 16 32 128 256 512; do
    sips -z "$SIZE" "$SIZE" "$ICON_TMP/icon.png" --out "$ICONSET/icon_${SIZE}x${SIZE}.png" >/dev/null
    DOUBLE=$((SIZE * 2))
    sips -z "$DOUBLE" "$DOUBLE" "$ICON_TMP/icon.png" --out "$ICONSET/icon_${SIZE}x${SIZE}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/LoreLens.icns"
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' lorelens/Cargo.toml | head -n 1)"
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
    <key>CFBundleName</key><string>LoreLens</string>
    <key>CFBundleDisplayName</key><string>LoreLens</string>
    <key>CFBundleIdentifier</key><string>kr.co.zenogrid.lorelens</string>
    <key>CFBundleExecutable</key><string>LoreLens</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleIconFile</key><string>LoreLens.icns</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF
plutil -lint "$APP/Contents/Info.plist"
if [[ -f "$APP/Contents/MacOS/dis/lore" ]]; then
    codesign --force --sign - "$APP/Contents/MacOS/dis/lore"
fi
codesign --force --sign - "$APP"
echo "Built $APP"
