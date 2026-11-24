# Equaform-8 (RomuEq) - Session State

**Date:** 2026-11-24
**Version:** v1.0.0 (published on GitHub Releases)
**Maturity Audit:** 8.2 / 10 (baseline was 6.8 / 10)

## Release Assets

- `Equaform-8-v1.0.0.dmg` + `.dmg.sha256` (macOS Apple Silicon)
- `Equaform-8-Plugins-Windows-v1.0.0.exe` + `.exe.sha256` (Windows x64)

## Platform Scope

- **macOS (Apple Silicon):** Plugins (AUv2, VST3, CLAP) + Standalone app (`Equaform-8.app` + CoreAudio HAL `Equaform.driver`) in combined DMG via `./scripts/package-distribution-plugins.sh <version>`.
- **Windows (x64):** Plugins only (VST3, CLAP + 15 factory presets) in Inno Setup 6 installer (`scripts/distribution/windows/installer.iss`); no standalone app.

## Test & Lint Status

- 448 tests passed, 0 failed, 2 ignored (hardware diagnostics)
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings
- `cargo fmt --all -- --check`: clean

## CI/CD Workflows

- `ci.yml`: 3-OS matrix + `cargo-llvm-cov` >= 90% core DSP/engine gate
- `release.yml`: unified atomic macOS + Windows release
- `nightly.yml`: weekly portable crate health check
- `security.yml`: dependency review and `cargo deny` audit

## Real-Time Safety Guarantees

- `LockFreeTripleBuffer<EqParameters>` for parameter passing
- Zero allocations in audio hot path
- Zero panics on channel mismatch in `AudioBuffer` / `EngineLifecycle::process`
- Zero `unwrap`/`expect` in `crates/equaform-gui/src/standalone.rs`
- Atomic CPAL error counters
- Adaptive GUI throttling (60 / 15 / 0 FPS)
- FFT idle sleep (-90 / -84 dBFS Schmitt trigger)

## Recent Verified Commits

| SHA | Summary |
|-----|---------|
| `d37165b` | `docs(readme): document macOS vs Windows platform scope and local DMG packaging` |
| `889e65e` | `fix(windows): align MSVC runtime library to static /MT for VST3 wrapper` |
| `4aa8ec8` | `fix(engine,common): return false instead of panicking on channel mismatch in process` |
| `4ca9d21` | `fix(windows): remove invalid createallsubdirs flag on flat preset glob` (`v1.0.0` tag) |
| `0803449` | `refactor(gui): eliminate unwrap and expect calls in standalone engine client` (local) |

## Operational Constraints

- Gate A: Git commit. Gate B: Git push (keep local by default to preserve CI runner quotas).
- 100% English repository.
- No em-dashes in commit messages.
- No `ADR-xxx` references in source code comments.
