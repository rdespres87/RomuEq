#!/bin/bash
# ==============================================================================
# install-user-plugins.command
# Installs Equaform-8 plugins directly to ~/Library/Audio/Plug-Ins/
# (Does NOT require administrator / root password)
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "============================================================"
echo " Equaform-8 User-Space Plugin Installer"
echo "============================================================"
echo ""
echo "Installing plugins to: ${HOME}/Library/Audio/Plug-Ins/..."

USER_AU_DIR="${HOME}/Library/Audio/Plug-Ins/Components"
USER_VST3_DIR="${HOME}/Library/Audio/Plug-Ins/VST3"
USER_CLAP_DIR="${HOME}/Library/Audio/Plug-Ins/CLAP"
USER_PRESETS_DIR="${HOME}/.equaform/presets"

mkdir -p "${USER_AU_DIR}" "${USER_VST3_DIR}" "${USER_CLAP_DIR}" "${USER_PRESETS_DIR}"

# Remove previous installations
rm -rf "${USER_AU_DIR}/Equaform-8.component" "${USER_AU_DIR}/RomuEq.component"
rm -rf "${USER_VST3_DIR}/Equaform-8.vst3" "${USER_VST3_DIR}/RomuEq.vst3"
rm -rf "${USER_CLAP_DIR}/Equaform-8.clap" "${USER_CLAP_DIR}/RomuEq.clap"

# Copy bundles
if [[ -d "${SCRIPT_DIR}/Equaform-8.component" ]]; then
    echo "  - Installing Equaform-8.component..."
    cp -R "${SCRIPT_DIR}/Equaform-8.component" "${USER_AU_DIR}/"
    xattr -rd com.apple.quarantine "${USER_AU_DIR}/Equaform-8.component" 2>/dev/null || true
fi

if [[ -d "${SCRIPT_DIR}/Equaform-8.vst3" ]]; then
    echo "  - Installing Equaform-8.vst3..."
    cp -R "${SCRIPT_DIR}/Equaform-8.vst3" "${USER_VST3_DIR}/"
    xattr -rd com.apple.quarantine "${USER_VST3_DIR}/Equaform-8.vst3" 2>/dev/null || true
fi

if [[ -d "${SCRIPT_DIR}/Equaform-8.clap" ]]; then
    echo "  - Installing Equaform-8.clap..."
    cp -R "${SCRIPT_DIR}/Equaform-8.clap" "${USER_CLAP_DIR}/"
    xattr -rd com.apple.quarantine "${USER_CLAP_DIR}/Equaform-8.clap" 2>/dev/null || true
fi

# Copy presets
if [[ -d "${SCRIPT_DIR}/Presets" ]]; then
    echo "  - Installing factory presets to ~/.equaform/presets/..."
    cp -R "${SCRIPT_DIR}/Presets/"* "${USER_PRESETS_DIR}/" 2>/dev/null || true
fi

# Notify CoreAudio
for DIR in "${USER_AU_DIR}" "${USER_VST3_DIR}" "${USER_CLAP_DIR}"; do
    if [ -d "$DIR" ]; then
        touch "$DIR" 2>/dev/null || true
    fi
done
killall -9 AudioComponentRegistrar 2>/dev/null || true

echo ""
echo "Done! Equaform-8 plugins are now installed in your user directory."
echo "You can now launch your DAW and scan for plugins."
echo ""
read -p "Press [Enter] to exit..."
exit 0
