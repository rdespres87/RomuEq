#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "==> Building native CoreAudio HAL driver..."
chmod +x "$SCRIPT_DIR/build-driver.sh"
"$SCRIPT_DIR/build-driver.sh"

echo "==> Building romueq-standalone in release mode..."
cargo build --package romueq-standalone --release --manifest-path "$ROOT_DIR/Cargo.toml"

APP_NAME="Equaform-8"
BUNDLE_DIR="$ROOT_DIR/target/release/bundle"
APP_DIR="$BUNDLE_DIR/$APP_NAME.app"

echo "==> Packaging $APP_DIR..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

cp "$ROOT_DIR/target/release/romueq-standalone" "$APP_DIR/Contents/MacOS/$APP_NAME"
chmod +x "$APP_DIR/Contents/MacOS/$APP_NAME"

if [ -f "$ROOT_DIR/crates/romueq-standalone/resources/RomuEq.icns" ]; then
    cp "$ROOT_DIR/crates/romueq-standalone/resources/RomuEq.icns" "$APP_DIR/Contents/Resources/Equaform-8.icns"
fi

cat << 'EOF' > "$APP_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Equaform-8</string>
    <key>CFBundleIconFile</key>
    <string>Equaform-8</string>
    <key>CFBundleIdentifier</key>
    <string>com.equaform.audio.equalizer.app</string>
    <key>CFBundleName</key>
    <string>Equaform-8</string>
    <key>CFBundleDisplayName</key>
    <string>Equaform-8</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>Equaform-8 nécessite l'accès au flux audio pour égaliser le son de votre Mac en temps réel.</string>
</dict>
</plist>
EOF

codesign --force --deep -s - "$APP_DIR"
echo "==> Application packaged and signed successfully: $APP_DIR"
