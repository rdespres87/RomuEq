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
- **Verification**:
  - `cargo test --workspace` (193 tests passing: all unit/integration tests + doctests).
  - `cargo check --target x86_64-pc-windows-msvc -p equaform-plugin` (100% clean with zero warnings).
  - `cargo clippy --workspace --all-targets -- -D warnings` (clean).
  - `cargo fmt --check` (clean).
- **Git Commits**:
  - `1c901d2` — `ci: modernize multiplatform GitHub Actions workflows and automated dual releases`
  - Current branch: `main`

## Next Logical Action
1. Push to GitHub (`git push origin main` or create/push a tag `git tag v1.0.0 && git push origin v1.0.0`) to trigger automated CI matrix and/or automatic dual release publishing.
2. Provide the macOS DMG (`target/release/dist/Equaform-8-Plugins-v1.0.0.dmg`) to your musician friend for testing on macOS.
3. Proceed to Option A (DSP Pro Features: Mid/Side processing, Band Listen/Solo mode, and Linear-phase EQ) or Option B (Frequency piano keyboard and vector resizable GUI).
