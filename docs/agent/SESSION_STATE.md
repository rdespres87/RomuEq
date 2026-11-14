# Session State - Equaform-8 (Equaform)

**Date:** 2026-11-14  
**Status:** ALL TESTS GREEN (193 tests passing), CROSS-PLATFORM WINDOWS PORT & MACOS PACKAGING COMPLETE, 100% CI AUTOMATED

## Multiplatform DAW Plugins Architecture (macOS & Windows)
- **Plugin Targets:** CLAP (`Equaform-8.clap`), VST3 (`Equaform-8.vst3`), AUv2 (`Equaform-8.component`, macOS only).
- **Scope Restriction:** Standalone application (`Equaform-8.app`) and HAL driver (`Equaform.driver`) strictly excluded (reserved for personal use).
- **Editor Abstraction Layer (`crates/equaform-plugin/src/editor/`):**
  - Cross-platform `PluginEditor` trait (`create`, `set_parent`, `show`, `hide`, `destroy`).
  - macOS backend: `src/editor/cocoa.rs` (`NSView` parent, Metal graphics pipeline via `wgpu`, Retina scaling).
  - Windows backend: `src/editor/win32.rs`:
    - Win32 child window (`WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN`) parented to host `HWND`.
    - `wgpu` graphics pipeline targeting DirectX 12 / Vulkan primary, with Direct3D 12 WARP software rasterizer fallback for VMs/RDP.
    - Full `WndProc` event pump: mouse position, mouse buttons with `SetCapture`/`ReleaseCapture` to prevent stuck drag gestures outside window boundaries, mouse wheel, keyboard input with `GetKeyState` modifier synchronization (`Shift`, `Ctrl`, `Alt`).
    - Dynamic Per-Monitor V2 High-DPI scaling via `GetDpiForWindow` and `WM_DPICHANGED` window/swapchain resizing.
    - 60 FPS redraw timer (`SetTimer` 16ms).
- **Preset Directory Resolution (`crates/equaform-gui/src/presets_io.rs`):**
  - Windows: Resolves to `%APPDATA%\Equaform\presets\` (with `%USERPROFILE%` and `C:\ProgramData` fallbacks).
  - macOS: Resolves to `~/.equaform/presets/`.

## Distribution Pipelines
1. **macOS Distribution Package (`scripts/package-distribution-plugins.sh`):**
   - Output: `target/release/dist/Equaform-8-Plugins-v1.0.0.dmg` (21 MB, UDZO) with SHA-256.
   - Modular `productbuild` installer `Equaform-8-Plugins-Installer.pkg` (AUv2, VST3, CLAP, Presets).
   - Automated Gatekeeper quarantine removal (`xattr -rd com.apple.quarantine`) and CoreAudio cache reset (`killall -9 AudioComponentRegistrar`).
   - Tester guide, quick start, 1-click Gatekeeper helper, uninstaller, and manual user-space fallback.
2. **Windows Distribution Package (`scripts/distribution/windows/installer.iss`):**
   - Inno Setup 6 installer script producing `Equaform-8-Plugins-Windows-v1.0.0.exe`.
   - 64-bit architecture enforcement (`x64compatible`).
   - Targets `{commoncf64}\VST3`, `{commoncf64}\CLAP`, and `%APPDATA%\Equaform\presets\`.
   - VST3 dependency auto-installs CLAP engine companion even on custom selective installs.
   - Clean Windows uninstallation via `{autopf}\Equaform\unins000.exe`.
3. **Automated CI/CD & Atomic Dual Release Workflow (`.github/workflows/`):**
   - `.github/workflows/ci.yml`: 3-OS matrix (`macos-latest` Apple Silicon, `windows-latest` MSVC x64, `ubuntu-latest` Linux with ALSA headers). Checks formatting, runs workspace tests and Clippy, and builds/verifies codesigned plugin bundles. Strict `contents: read` permissions.
   - `.github/workflows/release.yml`: Unified atomic multiplatform release pipeline triggered on `v*` tags or `workflow_dispatch`. Concurrently builds macOS `.dmg` and Windows `.exe` installers with SHA-256 checksums, then publishes all 4 assets together in a single atomic GitHub Release.
   - `.github/workflows/nightly.yml`: Weekly scheduled Linux validation of portable workspace crates.
   - `.github/workflows/security.yml`: Dependency review on pull requests and automated `cargo deny` check.
