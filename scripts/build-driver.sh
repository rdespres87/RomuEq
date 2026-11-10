#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DRIVER_SRC="$ROOT_DIR/driver/romueq-hal/src/RomuEqDriver.c"
DRIVER_PLIST="$ROOT_DIR/driver/romueq-hal/Info.plist"
BUILD_DIR="$ROOT_DIR/target/driver-build"
PKG_ROOT="$BUILD_DIR/pkgroot"
DRIVER_BUNDLE="$PKG_ROOT/Library/Audio/Plug-Ins/HAL/RomuEq.driver"
OUTPUT_PKG="$ROOT_DIR/crates/romueq-standalone/resources/RomuEqDriver.pkg"

echo "==> Compiling native CoreAudio HAL driver (RomuEq.driver)..."
rm -rf "$BUILD_DIR"
mkdir -p "$DRIVER_BUNDLE/Contents/MacOS"

clang -std=c11 -O3 -Wall -Wextra -Werror \
    -arch arm64 -arch x86_64 \
    -mmacosx-version-min=11.0 \
    -bundle \
    -framework CoreAudio \
    -framework CoreFoundation \
    "$DRIVER_SRC" \
    -o "$DRIVER_BUNDLE/Contents/MacOS/RomuEq"

cp "$DRIVER_PLIST" "$DRIVER_BUNDLE/Contents/Info.plist"

echo "==> Signing RomuEq.driver bundle..."
codesign --force --deep -s - "$DRIVER_BUNDLE"

echo "==> Packaging RomuEqDriver.pkg installer..."
pkgbuild \
    --root "$PKG_ROOT" \
    --identifier "com.romueq.audio.driver.pkg" \
    --version "1.0.0" \
    --install-location "/" \
    "$OUTPUT_PKG"

echo "==> Native RomuEqDriver.pkg built successfully: $OUTPUT_PKG"

