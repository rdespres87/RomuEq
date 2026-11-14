# Session State - Equaform-8 (Equaform)

**Date:** 2026-11-14  
**Status:** ALL TESTS GREEN (193 tests passing), COMMERCIAL BRANDING "Equaform-8" INSTALLED, BUNDLES SIGNED & PACKAGED, DISTRIBUTION DMG V1.0.0 READY FOR BETA TESTING

## Commercial Distribution Packaging (Option C - DAW Plugins)
- **Target Audience:** Musician / producer beta testers on macOS (Apple Silicon M1/M2/M3/M4, macOS 12+).
- **Scope Restriction:** Standalone application (`Equaform-8.app`) and HAL driver (`Equaform.driver`) strictly excluded (reserved for personal use).
- **Distribution Deliverables:**
  - `target/release/dist/Equaform-8-Plugins-v1.0.0.dmg` (21 MB, UDZO compressed).
  - SHA-256 Checksum recorded in `target/release/dist/Equaform-8-Plugins-v1.0.0.dmg.sha256`.
- **Installer Architecture (`scripts/distribution/`):**
  - Modular `productbuild` package (`Equaform-8-Plugins-Installer.pkg`) with component selection: AUv2 (`equaform-au.pkg`), VST3 (`equaform-vst3.pkg`), CLAP (`equaform-clap.pkg`), and Core/Presets (`equaform-core.pkg`).
  - Native Dark/Light mode adaptive styling (`color-scheme: light dark`) using Apple system color variables.
  - `preinstall`: Cleans legacy `RomuEq` bundles and removes conflicting user-space duplicate plugins.
  - `postinstall`: Strips Gatekeeper quarantine (`xattr -rd com.apple.quarantine`), sets system permissions (`root:wheel 755`), resets CoreAudio cache (`killall -9 AudioComponentRegistrar`), and synchronizes 15 factory presets to `~/.equaform/presets/` with console user ownership (`$CONSOLE_USER:staff`).
- **DMG Contents:**
  - `Equaform-8-Plugins-Installer.pkg`
  - `README_TESTER.txt` (Concise QuickStart in English)
  - `TEST_GUIDE_MAC.md` (Comprehensive English guide with Gatekeeper unlocking, DAW rescan procedures for Logic Pro, Ableton, Bitwig, Reaper, Cubase, and 5-point feedback checklist)
  - `Open-Installer-Gatekeeper.command` (1-click helper script copying `.pkg` to `/tmp` and removing quarantine)
  - `Uninstall-Equaform-Plugins.command` (1-click clean uninstaller purging bundles, presets, and package receipts)
  - `Manual-Install/` (Raw bundles and non-admin user-space install script `install-user-plugins.command`)
- **DAW Automation Compliance:**
  - All 51 parameters exposed to DAWs translated to English (`Master Gain`, `Band 1..8 Active`, `Band 1..8 Frequency`, `Band 1..8 Q Factor`, `Band 1..8 Slope`).
  - Unit test `test_all_51_param_metadata_english` verifies all 51 parameters in ASCII English.

## Commercial Branding Transition
- **Official Commercial Product Name:** `Equaform-8`
- **Vendor / Manufacturer:** `Equaform Audio`
- **Tagline:** *"The Geometry of Pure Balance"*
- **CoreAudio HAL Driver:**
  - Driver Bundle: `/Library/Audio/Plug-Ins/HAL/Equaform.driver`
  - Audio Device Name: `Equaform-8`
  - Audio Device Manufacturer: `Equaform Audio`
  - Package Installer: `EquaformDriver.pkg`
  - Factory Entry Point: `EquaformDriver_Create`
- **Plugin Metadata & Identifiers:**
  - CLAP / VST3 / AU Identifier: `com.equaform.audio.equalizer`
  - AUv2 Manufacturer Code: `Eqfm` (Equaform Audio)
  - AUv2 Subtype Code: `Eqf8` (Equaform-8)
  - AUv2 Type: `aufx`
- **Installed Artifacts:**
  - `~/Library/Audio/Plug-Ins/CLAP/Equaform-8.clap`
  - `~/Library/Audio/Plug-Ins/VST3/Equaform-8.vst3`
  - `~/Library/Audio/Plug-Ins/Components/Equaform-8.component`
  - `target/release/bundle/Equaform-8.app`
