#!/usr/bin/env bash
# ==============================================================================
# package-distribution-plugins.sh
# Builds and packages Equaform-8 DAW plugins (CLAP, VST3, AUv2) and factory
# presets into a production-grade macOS .pkg installer and distributable DMG.
#
# NOTE: Standalone app (Equaform-8.app) and CoreAudio HAL driver (Equaform.driver)
#       are STRICTLY EXCLUDED from this package.
# ==============================================================================

set -euo pipefail

VERSION="1.0.0"
BUNDLE_IDENTIFIER_BASE="com.equaform.audio.equalizer"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

RELEASE_DIR="${ROOT_DIR}/target/release"
TARGET_BUNDLE_DIR="${RELEASE_DIR}/bundle"
CMAKE_BUILD_DIR="${ROOT_DIR}/target/clap-wrapper-build"

STAGING_DIR="${RELEASE_DIR}/dist-staging"
PKG_STAGING_DIR="${STAGING_DIR}/pkg"
DMG_STAGING_DIR="${STAGING_DIR}/dmg"
DIST_OUT_DIR="${RELEASE_DIR}/dist"

CLAP_BUNDLE="${TARGET_BUNDLE_DIR}/Equaform-8.clap"
VST3_BUNDLE="${TARGET_BUNDLE_DIR}/Equaform-8.vst3"
AU_BUNDLE="${TARGET_BUNDLE_DIR}/Equaform-8.component"

DIST_RESOURCES_DIR="${ROOT_DIR}/scripts/distribution"

echo "========================================================================"
echo " Equaform-8 DAW Plugins Distribution Packager (v${VERSION})"
echo "========================================================================"

# --- Preflight Dependency Checks ---
echo "==> [0/8] Checking build tool prerequisites..."
for tool in cargo cmake ninja codesign pkgbuild productbuild hdiutil; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "ERROR: Required tool '$tool' is not installed or not in PATH." >&2
        exit 1
    fi
done
echo "    All build prerequisites satisfied."

# --- Clean Staging Dirs ---
rm -rf "${STAGING_DIR}" "${DIST_OUT_DIR}"
mkdir -p "${TARGET_BUNDLE_DIR}" "${PKG_STAGING_DIR}/components" "${DMG_STAGING_DIR}" "${DIST_OUT_DIR}"

# --- Step 1: Compile equaform-plugin in release mode ---
echo "==> [1/8] Compiling equaform-plugin (CLAP dynamic library)..."
cargo build --manifest-path "${ROOT_DIR}/Cargo.toml" --release -p equaform-plugin

DYLIB_PATH="${RELEASE_DIR}/libequaform_plugin.dylib"
if [[ ! -f "${DYLIB_PATH}" ]]; then
    echo "ERROR: Expected plugin dylib not found at ${DYLIB_PATH}" >&2
    exit 1
fi

# --- Step 2: Assemble Equaform-8.clap bundle ---
echo "==> [2/8] Assembling Equaform-8.clap bundle..."
rm -rf "${CLAP_BUNDLE}"
mkdir -p "${CLAP_BUNDLE}/Contents/MacOS"
cp "${DYLIB_PATH}" "${CLAP_BUNDLE}/Contents/MacOS/Equaform-8"
printf "BNDL????" > "${CLAP_BUNDLE}/Contents/PkgInfo"

cat > "${CLAP_BUNDLE}/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>Equaform-8</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_IDENTIFIER_BASE}.clap</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Equaform-8</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
</dict>
</plist>
EOF

codesign --force --deep -s - "${CLAP_BUNDLE}"

# --- Step 3: Build VST3 and AUv2 via clap-wrapper ---
echo "==> [3/8] Wrapping into VST3 and AUv2 via clap-wrapper..."
mkdir -p "${CMAKE_BUILD_DIR}/bundles"
cmake -G Ninja \
    -S "${ROOT_DIR}/driver/clap-wrapper-equaform" \
    -B "${CMAKE_BUILD_DIR}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_DEPLOYMENT_TARGET=12.0 \
    -DEQUAFORM_CLAP_BUNDLE="${CLAP_BUNDLE}" \
    -DEQUAFORM_BUNDLE_OUT_DIR="${CMAKE_BUILD_DIR}/bundles"

cmake --build "${CMAKE_BUILD_DIR}" --target equaform_vst3 equaform_auv2

rm -rf "${VST3_BUNDLE}" "${AU_BUNDLE}"
cp -R "${CMAKE_BUILD_DIR}/bundles/Equaform-8.vst3" "${VST3_BUNDLE}"
cp -R "${CMAKE_BUILD_DIR}/bundles/Equaform-8.component" "${AU_BUNDLE}"

# Guarantee fresh CLAP replacement inside wrapper bundles
rm -rf "${VST3_BUNDLE}/Contents/PlugIns/Equaform-8.clap" "${AU_BUNDLE}/Contents/PlugIns/Equaform-8.clap"
cp -R "${CLAP_BUNDLE}" "${VST3_BUNDLE}/Contents/PlugIns/Equaform-8.clap"
cp -R "${CLAP_BUNDLE}" "${AU_BUNDLE}/Contents/PlugIns/Equaform-8.clap"

# Apply the AUv2 Info.plist containing AudioComponents dictionary
AUV2_INFO_PLIST="${CMAKE_BUILD_DIR}/equaform_auv2-build-helper-output/auv2_Info.plist"
if [[ ! -f "${AUV2_INFO_PLIST}" ]]; then
    echo "ERROR: Expected AUv2 Info.plist not found at ${AUV2_INFO_PLIST}" >&2
    exit 1
fi
cp "${AUV2_INFO_PLIST}" "${AU_BUNDLE}/Contents/Info.plist"

# --- Step 4: Strict Inside-Out Ad-Hoc Codesigning ---
echo "==> [4/8] Signing plugin bundles (inside-out ad-hoc)..."
codesign --force -s - "${VST3_BUNDLE}/Contents/PlugIns/Equaform-8.clap"
codesign --force -s - "${VST3_BUNDLE}"
codesign --force -s - "${AU_BUNDLE}/Contents/PlugIns/Equaform-8.clap"
codesign --force -s - "${AU_BUNDLE}"

codesign --verify --deep --strict "${CLAP_BUNDLE}"
codesign --verify --deep --strict "${VST3_BUNDLE}"
codesign --verify --deep --strict "${AU_BUNDLE}"
echo "    Plugin bundles successfully signed and strictly verified."

# --- Step 5: Export the 15 Factory Presets ---
echo "==> [5/8] Exporting 15 binary factory presets..."
CORE_PRESETS_DIR="${PKG_STAGING_DIR}/root-core/Library/Application Support/Equaform/presets"
mkdir -p "${CORE_PRESETS_DIR}"
cargo run -p equaform-gui --example dump_factory_presets -- "${CORE_PRESETS_DIR}"

PRESET_COUNT=$(find "${CORE_PRESETS_DIR}" -type f -name "*.req" | wc -l | tr -d ' ')
if [[ "${PRESET_COUNT}" -lt 15 ]]; then
    echo "ERROR: Expected at least 15 presets, found ${PRESET_COUNT}" >&2
    exit 1
fi
echo "    Exported ${PRESET_COUNT} factory presets successfully."

# --- Step 6: Build Component Packages (pkgbuild) ---
echo "==> [6/8] Building modular component packages (pkgbuild)..."
export COPYFILE_DISABLE=1
find "${PKG_STAGING_DIR}" -name "._*" -delete 2>/dev/null || true
xattr -rc "${PKG_STAGING_DIR}" 2>/dev/null || true

# 6a. Core package (Presets + preinstall/postinstall hooks)
pkgbuild \
    --root "${PKG_STAGING_DIR}/root-core" \
    --scripts "${DIST_RESOURCES_DIR}/scripts" \
    --identifier "${BUNDLE_IDENTIFIER_BASE}.pkg.core" \
    --version "${VERSION}" \
    --install-location "/" \
    "${PKG_STAGING_DIR}/components/equaform-core.pkg"

# 6b. Audio Unit v2 package
mkdir -p "${PKG_STAGING_DIR}/root-au/Library/Audio/Plug-Ins/Components"
cp -R "${AU_BUNDLE}" "${PKG_STAGING_DIR}/root-au/Library/Audio/Plug-Ins/Components/"
pkgbuild \
    --root "${PKG_STAGING_DIR}/root-au" \
    --identifier "${BUNDLE_IDENTIFIER_BASE}.pkg.au" \
    --version "${VERSION}" \
    --install-location "/" \
    "${PKG_STAGING_DIR}/components/equaform-au.pkg"

# 6c. VST3 package
mkdir -p "${PKG_STAGING_DIR}/root-vst3/Library/Audio/Plug-Ins/VST3"
cp -R "${VST3_BUNDLE}" "${PKG_STAGING_DIR}/root-vst3/Library/Audio/Plug-Ins/VST3/"
pkgbuild \
    --root "${PKG_STAGING_DIR}/root-vst3" \
    --identifier "${BUNDLE_IDENTIFIER_BASE}.pkg.vst3" \
    --version "${VERSION}" \
    --install-location "/" \
    "${PKG_STAGING_DIR}/components/equaform-vst3.pkg"

# 6d. CLAP package
mkdir -p "${PKG_STAGING_DIR}/root-clap/Library/Audio/Plug-Ins/CLAP"
cp -R "${CLAP_BUNDLE}" "${PKG_STAGING_DIR}/root-clap/Library/Audio/Plug-Ins/CLAP/"
pkgbuild \
    --root "${PKG_STAGING_DIR}/root-clap" \
    --identifier "${BUNDLE_IDENTIFIER_BASE}.pkg.clap" \
    --version "${VERSION}" \
    --install-location "/" \
    "${PKG_STAGING_DIR}/components/equaform-clap.pkg"

# --- Step 7: Synthesize Master Installer Package (productbuild) ---
echo "==> [7/8] Synthesizing master installer package (productbuild)..."
INSTALLER_PKG="${DMG_STAGING_DIR}/Equaform-8-Plugins-Installer.pkg"
productbuild \
    --distribution "${DIST_RESOURCES_DIR}/Distribution.xml" \
    --package-path "${PKG_STAGING_DIR}/components" \
    --resources "${DIST_RESOURCES_DIR}/resources" \
    "${INSTALLER_PKG}"

echo "    Master installer package created at: ${INSTALLER_PKG}"

# --- Step 8: Build Distributable Disk Image (DMG) ---
echo "==> [8/8] Assembling DMG contents and building disk image..."

# Copy documentation and helper scripts to DMG root
cp "${DIST_RESOURCES_DIR}/dmg-root/README_TESTER.txt" "${DMG_STAGING_DIR}/"
cp "${DIST_RESOURCES_DIR}/dmg-root/TEST_GUIDE_MAC.md" "${DMG_STAGING_DIR}/"
cp "${DIST_RESOURCES_DIR}/dmg-root/Open-Installer-Gatekeeper.command" "${DMG_STAGING_DIR}/"
cp "${DIST_RESOURCES_DIR}/dmg-root/Uninstall-Equaform-Plugins.command" "${DMG_STAGING_DIR}/"
chmod +x "${DMG_STAGING_DIR}"/*.command

# Prepare Manual-Install folder as fallback
MANUAL_DIR="${DMG_STAGING_DIR}/Manual-Install"
mkdir -p "${MANUAL_DIR}/Presets"
cp -R "${AU_BUNDLE}" "${MANUAL_DIR}/"
cp -R "${VST3_BUNDLE}" "${MANUAL_DIR}/"
cp -R "${CLAP_BUNDLE}" "${MANUAL_DIR}/"
cp -R "${CORE_PRESETS_DIR}/"* "${MANUAL_DIR}/Presets/"
cp "${DIST_RESOURCES_DIR}/dmg-root/install-user-plugins.command" "${MANUAL_DIR}/"
chmod +x "${MANUAL_DIR}/install-user-plugins.command"

# Create final DMG
DMG_PATH="${DIST_OUT_DIR}/Equaform-8-Plugins-v${VERSION}.dmg"
echo "    Packaging DMG: ${DMG_PATH}..."
hdiutil create \
    -volname "Equaform-8-Plugins" \
    -srcfolder "${DMG_STAGING_DIR}" \
    -ov \
    -format UDZO \
    "${DMG_PATH}"

# Generate SHA256 checksum
shasum -a 256 "${DMG_PATH}" > "${DMG_PATH}.sha256"

echo ""
echo "========================================================================"
echo " BUILD & PACKAGING COMPLETE!"
echo "========================================================================"
echo " Distributable DMG  : ${DMG_PATH}"
echo " File Size          : $(du -sh "${DMG_PATH}" | cut -f1)"
echo " SHA-256 Checksum   : $(cat "${DMG_PATH}.sha256")"
echo ""
echo " Contents inside DMG:"
echo "   - Equaform-8-Plugins-Installer.pkg"
echo "   - README_TESTER.txt"
echo "   - TEST_GUIDE_MAC.md"
echo "   - Open-Installer-Gatekeeper.command"
echo "   - Uninstall-Equaform-Plugins.command"
echo "   - Manual-Install/ (with raw bundles + install-user-plugins.command)"
echo "========================================================================"
