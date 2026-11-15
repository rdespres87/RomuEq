# Session State - Equaform-8 (Equaform)

**Date:** 2026-11-15  
**Status:** ALL TESTS GREEN (378 tests passing, 0 failures), ADR-007 REAL-TIME SAFETY AUDIT IMPLEMENTED, ZERO ALLOCATION IN DSP/AUDIO CALLBACK, ADAPTIVE GUI THROTTLING & ENERGY OPTIMIZATION

## Performance, Energy & Memory Optimization (Standalone & DAW Plugins)
- **Zero-Allocation Spectrum Analysis (`crates/equaform-analysis/src/lib.rs` & `crates/equaform-gui/`):**
  - Added `process_ref(&mut self, ...)` and `process_into(&mut self, ...)` reusing internal frequency bin and EMA smoothing buffers without vector reallocations.
  - Eradicated 128-point dummy heap vector allocation in `EngineClient::analysis_snapshot()`.
  - Added persistent pre-allocated `analysis` instances and `sample_buffer` in `StandaloneRunner` (`crates/equaform-gui/src/lib.rs`), `CocoaEditor` (`crates/equaform-plugin/src/editor/cocoa.rs`), and `Win32Editor` (`crates/equaform-plugin/src/editor/win32.rs`).
- **Adaptive Background GUI Throttling (60 FPS -> 15 FPS -> 0 FPS):**
  - **Zero Input Latency Priority:** Mouse movement, clicks, drags, or pending events immediately trigger full 60 FPS redraw with zero input lag.
  - **Standalone Mode:** 60 FPS while focused; throttles to 15 FPS (`request_repaint_after(66ms)`) when unfocused/background; drops to 0 FPS when minimized/hidden.
  - **DAW Plugin Mode (Cocoa & Win32):** Active check `is_key || is_main || mouse_inside || has_pending_events` ensures 60 FPS during user interaction; throttles to 15 FPS when inactive/unfocused; suspends redraws (0 FPS) when occluded, hidden, or minimized.
  - **Ring Buffer Resynchronization:** When waking up from 0 FPS dormancy (or when ring buffer fullness exceeds 50%), instant consumer flush (`consumer.clear()` & `fft_ring.clear()`) realigns spectrum visualization with live incoming sound, preventing temporal drift.
- **FFT Idle Sleep on Transport Stop & Silence:**
  - DAW transport stop detection via `!popped_any` counter ($\ge 15$ ticks without new audio blocks).
  - Schmitt trigger hysteresis (-90 dBFS sleep threshold, -84 dBFS wake threshold).
  - Bypasses FFT computation and logarithmic binning when audio is silent or transport is halted.

## Target Directory Optimization (Freeing 11.3 GB)
- Configured `Cargo.toml` with `[profile.dev] debug = 1` and `[profile.dev.package."*"] debug = 0` (and `profile.test`), stripping DWARF symbols from 300+ third-party dependencies while preserving full project debuggability.
- Purged cross-compilation caches and installed `cargo-sweep` v0.8.0.
- Created `scripts/clean-target.sh`. Total `target/` directory size reduced from 14 GB to 2.7 GB (81% reduction; `target/debug` reduced from 9.4 GB to 1.2 GB).

## ADR-007 Real-Time Safety & Architecture Hardening
- **Lock-Free Triple Buffer (`crates/equaform-common/src/triple_buffer.rs`)**:
  - `LockFreeTripleBuffer<EqParameters>` using contiguous pre-allocated slots (`[UnsafeCell<T>; 3]`) and atomic index swap (`AtomicU8` with dirty bit).
  - Wait-free $O(1)$ writes from GUI and lock-free $O(1)$ reads in audio thread. Zero allocation, zero deallocation, zero OS mutexes.
  - Replaced legacy `std::sync::mpsc::sync_channel(256)` in `crates/equaform-gui/src/audio.rs`.
- **CPAL Error Handlers Sanitization**:
  - Replaced synchronous `eprintln!` I/O in CPAL stream error callbacks with atomic counter `stream_errors: Arc<AtomicUsize>`.
- **Zero-Allocation Automation & DSP Continuity (`automation.rs` & `lifecycle.rs`)**:
  - Eradicated 8,192 heap allocations per 512-sample block in `AutomationValueProvider` (`value_at_sample` and `last_value_in_block`).
  - Fixed semantic block-relative sample offset indexing ($0 \le \text{offset} < \text{block\_size}$).
  - Ensured parameter persistence across blocks: updating `self.cached_coeffs[band_idx] = *coeff_slot` and auto-gain re-evaluation on automated blocks to prevent coefficient dropouts.
- **Context Structs**:
  - `AudioRenderContext<'a>` grouping 17 arguments in `process_output_block`.
  - `WorkspaceUiContext<'a>` grouping 10 arguments in `render_workspace` across 4 platforms (Standalone, Headless tests, Cocoa/Metal, Win32/DirectX).
- **Metadata Factorization**:
  - Reduced 540 lines of repetitive match arms in `crates/equaform-common/src/parameter.rs` via $O(1)$ band arithmetic mapping.
- **CoreAudio Portability**:
  - Eradicated hardcoded localized device names (`"Haut-parleurs"`). Dynamic physical device resolution via `kAudioHardwarePropertyDefaultOutputDevice`.
- **Deterministic Audio Tests**:
  - Converted `test_realtime_threaded_streaming` from passive sleep to deterministic step-based discrete frame simulation (<10ms).

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
