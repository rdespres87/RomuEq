#!/bin/bash
# ==============================================================================
# Uninstall-Equaform-Plugins.command
# Clean uninstallation of Equaform-8 plugins, system presets, and package receipts
# ==============================================================================

set -euo pipefail

echo "============================================================"
echo " Equaform-8 Plugins Uninstaller"
echo "============================================================"
echo ""
echo "This script will remove Equaform-8 plugins from /Library/Audio/Plug-Ins/."
echo "Administrator privileges are required."
echo ""

sudo -v || { echo "Authentication failed. Exiting."; exit 1; }

echo "Removing system-wide plugins..."
sudo rm -rf "/Library/Audio/Plug-Ins/Components/Equaform-8.component"
sudo rm -rf "/Library/Audio/Plug-Ins/VST3/Equaform-8.vst3"
sudo rm -rf "/Library/Audio/Plug-Ins/CLAP/Equaform-8.clap"
sudo rm -rf "/Library/Application Support/Equaform"

echo "Forgetting package receipts..."
for pkg in core au vst3 clap; do
    sudo pkgutil --forget "com.equaform.audio.equalizer.pkg.${pkg}" 2>/dev/null || true
done

echo "Removing user-level plugins if present..."
rm -rf "${HOME}/Library/Audio/Plug-Ins/Components/Equaform-8.component"
rm -rf "${HOME}/Library/Audio/Plug-Ins/VST3/Equaform-8.vst3"
rm -rf "${HOME}/Library/Audio/Plug-Ins/CLAP/Equaform-8.clap"

echo "Refreshing CoreAudio registration..."
for DIR in \
    "/Library/Audio/Plug-Ins/Components" \
    "/Library/Audio/Plug-Ins/VST3" \
    "/Library/Audio/Plug-Ins/CLAP"; do
    if [ -d "$DIR" ]; then
        sudo touch "$DIR" 2>/dev/null || true
    fi
done

sudo killall -9 AudioComponentRegistrar 2>/dev/null || true

echo ""
echo "Equaform-8 plugins have been successfully uninstalled."
echo "Note: Your personal presets in ~/.equaform/presets/ were kept safe."
echo ""
read -p "Press [Enter] to exit..."
exit 0
