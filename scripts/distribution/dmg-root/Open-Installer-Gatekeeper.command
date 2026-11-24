#!/bin/bash
# ==============================================================================
# Open-Installer-Gatekeeper.command
# Equaform-8 Beta Helper: Copies installer to writable temp folder,
# removes quarantine attribute, and launches installer.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG_PATH="${SCRIPT_DIR}/Equaform-8-Installer.pkg"

echo "============================================================"
echo " Equaform-8 Installer Gatekeeper Helper"
echo "============================================================"
echo ""

if [[ ! -f "${PKG_PATH}" ]]; then
    echo "ERROR: Could not find installer at: ${PKG_PATH}"
    echo "Please ensure this script is run from the mounted Equaform-8 DMG."
    read -p "Press [Enter] to exit..."
    exit 1
fi

TMP_DIR=$(mktemp -d /tmp/equaform-installer.XXXXXX)
PKG_COPY="${TMP_DIR}/Equaform-8-Installer.pkg"

echo "Preparing installer in temporary directory..."
cp "${PKG_PATH}" "${PKG_COPY}"

echo "Removing Gatekeeper quarantine attribute (com.apple.quarantine)..."
xattr -d com.apple.quarantine "${PKG_COPY}" 2>/dev/null || true

echo "Launching installer..."
open "${PKG_COPY}"

echo ""
echo "Done! The macOS Installer window should appear shortly."
sleep 2
exit 0
