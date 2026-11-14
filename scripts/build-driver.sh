#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DRIVER_SRC="$ROOT_DIR/driver/equaform-hal/src/EquaformDriver.c"
DRIVER_PLIST="$ROOT_DIR/driver/equaform-hal/Info.plist"
BUILD_DIR="$ROOT_DIR/target/driver-build"
PKG_ROOT="$BUILD_DIR/pkgroot"
DRIVER_BUNDLE="$PKG_ROOT/Library/Audio/Plug-Ins/HAL/Equaform.driver"
OUTPUT_PKG="$ROOT_DIR/crates/equaform-standalone/resources/EquaformDriver.pkg"

echo "==> Compiling native CoreAudio HAL driver (Equaform.driver)..."
rm -rf "$BUILD_DIR"
mkdir -p "$DRIVER_BUNDLE/Contents/MacOS"

clang -std=c11 -O3 -Wall -Wextra -Werror \
    -arch arm64 -arch x86_64 \
    -mmacosx-version-min=11.0 \
    -bundle \
    -framework CoreAudio \
    -framework CoreFoundation \
    "$DRIVER_SRC" \
    -o "$DRIVER_BUNDLE/Contents/MacOS/Equaform"

cp "$DRIVER_PLIST" "$DRIVER_BUNDLE/Contents/Info.plist"

echo "==> Signing Equaform.driver bundle..."
codesign --force --deep -s - "$DRIVER_BUNDLE"

echo "==> Packaging EquaformDriver.pkg installer..."
pkgbuild \
    --root "$PKG_ROOT" \
    --identifier "com.equaform.audio.driver.pkg" \
    --version "1.1.0" \
    --install-location "/" \
    "$OUTPUT_PKG"

echo "==> Native EquaformDriver.pkg built successfully: $OUTPUT_PKG"

