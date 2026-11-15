# Handoff - Equaform-8

## Current State
- **Cross-Platform DAW Plugins (macOS & Windows)**:
  - Both platforms supported with a unified codebase sharing DSP, analysis, egui workspace UI, and state serialization.
  - Standalone application and HAL audio driver strictly excluded from distribution.
- **macOS Distribution Deliverables**:
  - `target/release/dist/Equaform-8-Plugins-v1.0.0.dmg` (21 MB, UDZO) with SHA-256 checksum.
  - Automated inside-out ad-hoc codesigning, modular `productbuild` installer (`Equaform-8-Plugins-Installer.pkg`), and Apple `auval` verification passed (`AU VALIDATION SUCCEEDED`).
- **Windows Port & Packaging Deliverables**:
  - Architecture: 64-bit Windows 10 & 11 (`x86_64-pc-windows-msvc`).
  - Native Win32 UI embedding (`crates/equaform-plugin/src/editor/win32.rs`) with DirectX 12 / Vulkan `wgpu` surface, `SetCapture` mouse handling, `GetKeyState` keyboard modifier synchronization, and Per-Monitor V2 High-DPI scaling (`GetDpiForWindow`).
  - Inno Setup 6 configuration (`scripts/distribution/windows/installer.iss`) targeting standard 64-bit paths (`Common Files\VST3`, `Common Files\CLAP`, `%APPDATA%\Equaform\presets`).
  - GitHub Actions CI workflow (`.github/workflows/windows-release.yml`) for automated building and installer generation on `windows-latest`.
  - Comprehensive English testing documentation (`TEST_GUIDE_WINDOWS.md`, `README_WINDOWS.txt`).
- **Automated Multiplatform CI/CD & Atomic Dual Releases (`.github/workflows/`)**:
  - `ci.yml`: 3-OS matrix (`macos-latest` Apple Silicon, `windows-latest` MSVC x64, `ubuntu-latest` Linux). Tests formatting, portable Linux crates, Windows MSVC build/check, and full macOS bundles (CLAP, VST3, AUv2) with inside-out ad-hoc codesigning and strict verification.
  - `release.yml`: Unified atomic release pipeline. On `v*` tag push or manual `workflow_dispatch`, runs `build-macos` and `build-windows` in parallel, then executes `publish` on Ubuntu to attach all 4 files (`.dmg`, `.dmg.sha256`, `.exe`, `.exe.sha256`) into a single atomic GitHub Release.
  - `nightly.yml` & `security.yml`: Scheduled portable crate health and automated PR dependency / `cargo-deny` security checks.
- **ADR-007 Real-Time Hardening & Architecture Audit Refactoring**:
  - `LockFreeTripleBuffer<EqParameters>` implemented in `crates/equaform-common/src/triple_buffer.rs` (contiguous pre-allocated `[UnsafeCell; 3]`, atomic dirty-bit swap). Completely replaced `std::sync::mpsc::sync_channel` in `crates/equaform-gui/src/audio.rs`.
  - Atomic error counters for CPAL closures, eliminating blocking I/O (`eprintln!`) from audio thread.
  - Automation provider zero-allocation rewrite (`value_at_sample` and `last_value_in_block`), eliminating 8,192 heap allocations per block. Fixed sample offset semantics and ensured biquad coefficients persist cleanly across blocks.
  - Context structs: `AudioRenderContext` (17 arguments) and `WorkspaceUiContext` (10 arguments across Standalone, Tests, Cocoa/macOS, Win32/Windows).
  - Parameter metadata factorization in `parameter.rs` (-540 lines).
  - Portability: dynamic physical device resolution via CoreAudio HAL native `kAudioHardwarePropertyDefaultOutputDevice`, eliminating hardcoded French strings (`"Haut-parleurs"`).
  - Deterministic audio tests: discrete frame simulation in `audio.rs`, reducing test runtime from 1.2s to <10ms with zero flakiness.
- **Verification**:
  - `cargo test --workspace --all-features` (378 tests passing, 0 failures).
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings` (clean, 0 warnings).
  - `cargo fmt --all -- --check` (clean).
  - Apple `auval -v aufx Eqf8 Eqfm` (`AU VALIDATION SUCCEEDED`).
  - Automated CI Code Coverage via `cargo-llvm-cov` with $\ge 90\%$ line coverage enforced on core crates (`equaform-common`, `equaform-dsp`, `equaform-analysis`, `equaform-engine`).
- **Methodology & Standards**:
  - Spec-Driven TDD Protocol documented in `docs/development/TDD_METHODOLOGY.md` (Spec/ADR -> Red -> Green -> Refactor -> Gate).
- **Performance & Energy Optimizations**:
  - Zero-allocation spectrum processing (`process_ref`, `process_into`, static buffer reuse).
  - Adaptive GUI throttling (60 FPS active, 15 FPS unfocused, 0 FPS occluded/minimized with immediate ring-buffer resync).
  - Transport stop and silence detection with Schmitt trigger hysteresis (-90 dBFS / -84 dBFS), bypassing FFT computation during silence.
- **Git Commits**:
  - `cba6a6c` — `feat(audio, architecture): ADR-007 real-time safety, lock-free triple buffer, zero-allocation automation, and context structs`
  - `47c8e6a` — `chore(cargo): optimize target disk usage with selective debuginfo and sweep script`
  - `25b517c` — `perf(gui, plugin): adaptive background throttling, zero-allocation spectrum, and FFT idle sleep`
  - Current branch: `main`

## Next Logical Action
1. Test the freshly built and installed standalone application (`open target/release/bundle/Equaform-8.app`) or DAW plugins (in Logic Pro, Ableton Live, Bitwig, Reaper).
2. Proceed to Option A (DSP Pro Features: Mid/Side processing, Band Listen/Solo mode, and Linear-phase EQ) or Option B (Frequency piano keyboard and vector resizable GUI).
