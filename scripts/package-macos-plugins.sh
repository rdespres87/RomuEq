#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_BUNDLE_DIR="${ROOT_DIR}/target/release/bundle"
CMAKE_BUILD_DIR="${ROOT_DIR}/target/clap-wrapper-build"
CLAP_BUNDLE="${TARGET_BUNDLE_DIR}/RomuEq.clap"
VST3_BUNDLE="${TARGET_BUNDLE_DIR}/RomuEq.vst3"
AU_BUNDLE="${TARGET_BUNDLE_DIR}/RomuEq.component"

CLAP_INSTALL_DIR="${HOME}/Library/Audio/Plug-Ins/CLAP"
VST3_INSTALL_DIR="${HOME}/Library/Audio/Plug-Ins/VST3"
AU_INSTALL_DIR="${HOME}/Library/Audio/Plug-Ins/Components"

echo "==> [1/5] Building romueq-plugin (release)..."
cargo build --manifest-path "${ROOT_DIR}/Cargo.toml" --release -p romueq-plugin

DYLIB_PATH="${ROOT_DIR}/target/release/libromueq_plugin.dylib"
if [[ ! -f "${DYLIB_PATH}" ]]; then
    echo "ERROR: Expected dylib not found at ${DYLIB_PATH}" >&2
    exit 1
fi

echo "==> [2/5] Assembling RomuEq.clap bundle..."
rm -rf "${CLAP_BUNDLE}"
mkdir -p "${CLAP_BUNDLE}/Contents/MacOS"
cp "${DYLIB_PATH}" "${CLAP_BUNDLE}/Contents/MacOS/RomuEq"
printf "BNDL????" > "${CLAP_BUNDLE}/Contents/PkgInfo"

cat > "${CLAP_BUNDLE}/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>RomuEq</string>
    <key>CFBundleIdentifier</key>
    <string>com.romueq.plugin.clap</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>RomuEq</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
</dict>
</plist>
EOF

codesign --force --deep -s - "${CLAP_BUNDLE}"

echo "==> [3/5] Building RomuEq.vst3 and RomuEq.component via clap-wrapper..."
mkdir -p "${CMAKE_BUILD_DIR}/bundles"
cmake -G Ninja \
    -S "${ROOT_DIR}/driver/clap-wrapper-romueq" \
    -B "${CMAKE_BUILD_DIR}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_DEPLOYMENT_TARGET=12.0 \
    -DROMUEQ_CLAP_BUNDLE="${CLAP_BUNDLE}" \
    -DROMUEQ_BUNDLE_OUT_DIR="${CMAKE_BUILD_DIR}/bundles"

cmake --build "${CMAKE_BUILD_DIR}" --target romueq_vst3 romueq_auv2

rm -rf "${VST3_BUNDLE}" "${AU_BUNDLE}"
cp -R "${CMAKE_BUILD_DIR}/bundles/RomuEq.vst3" "${VST3_BUNDLE}"
cp -R "${CMAKE_BUILD_DIR}/bundles/RomuEq.component" "${AU_BUNDLE}"

# Ensure freshly rebuilt RomuEq.clap replaces any cached copy inside wrapper bundles
# when Ninja skips re-linking C++ targets during incremental Rust builds.
rm -rf "${VST3_BUNDLE}/Contents/PlugIns/RomuEq.clap" "${AU_BUNDLE}/Contents/PlugIns/RomuEq.clap"
cp -R "${CLAP_BUNDLE}" "${VST3_BUNDLE}/Contents/PlugIns/RomuEq.clap"
cp -R "${CLAP_BUNDLE}" "${AU_BUNDLE}/Contents/PlugIns/RomuEq.clap"

# Ensure the AUv2 Info.plist with AudioComponents metadata is always applied
# even if cmake configure regenerated the default bundle Info.plist and Ninja skipped PRE_BUILD.
AUV2_INFO_PLIST="${CMAKE_BUILD_DIR}/romueq_auv2-build-helper-output/auv2_Info.plist"
if [[ ! -f "${AUV2_INFO_PLIST}" ]]; then
    echo "ERROR: Expected AUv2 Info.plist not found at ${AUV2_INFO_PLIST}" >&2
    exit 1
fi
cp "${AUV2_INFO_PLIST}" "${AU_BUNDLE}/Contents/Info.plist"

echo "==> [4/5] Ad-hoc codesigning bundles (inside-out)..."
codesign --force -s - "${VST3_BUNDLE}/Contents/PlugIns/RomuEq.clap"
codesign --force -s - "${VST3_BUNDLE}"
codesign --force -s - "${AU_BUNDLE}/Contents/PlugIns/RomuEq.clap"
codesign --force -s - "${AU_BUNDLE}"

codesign --verify --deep --strict "${CLAP_BUNDLE}"
codesign --verify --deep --strict "${VST3_BUNDLE}"
codesign --verify --deep --strict "${AU_BUNDLE}"

echo "==> [5/5] Installing bundles to ~/Library/Audio/Plug-Ins/..."
mkdir -p "${CLAP_INSTALL_DIR}" "${VST3_INSTALL_DIR}" "${AU_INSTALL_DIR}"
rm -rf "${CLAP_INSTALL_DIR}/RomuEq.clap" "${VST3_INSTALL_DIR}/RomuEq.vst3" "${AU_INSTALL_DIR}/RomuEq.component"
cp -R "${CLAP_BUNDLE}" "${CLAP_INSTALL_DIR}/RomuEq.clap"
cp -R "${VST3_BUNDLE}" "${VST3_INSTALL_DIR}/RomuEq.vst3"
cp -R "${AU_BUNDLE}" "${AU_INSTALL_DIR}/RomuEq.component"

# Refresh CoreAudio AudioComponent cache so Logic/GarageBand/auval see updates immediately
touch "${AU_INSTALL_DIR}"
killall -9 AudioComponentRegistrar 2>/dev/null || true

echo ""
echo "Done! Installed:"
echo "  CLAP : ${CLAP_INSTALL_DIR}/RomuEq.clap"
echo "  VST3 : ${VST3_INSTALL_DIR}/RomuEq.vst3"
echo "  AUv2 : ${AU_INSTALL_DIR}/RomuEq.component"

