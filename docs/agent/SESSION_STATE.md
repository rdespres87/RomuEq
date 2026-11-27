# Equaform-8 (RomuEq) - Session State

**Date:** 2026-11-27
**Version:** v1.3.0 ("Linear-Phase FIR EQ", local) — `v1.1.1` live on GitHub Releases
**Maturity Audit:** **9.6 / 10** (**0 P0s, 0 P1s, 0 P2s**, 0 `allow(clippy::)`, 0 prod files > 1,000 LOC, 0 stale comments)

## Release Assets

- `Equaform-8-v1.3.0.dmg` + `.dmg.sha256` (macOS Apple Silicon)
- `Equaform-8-Plugins-Windows-v1.3.0.exe` + `.exe.sha256` (Windows x64)

## Platform Scope

- **macOS (Apple Silicon):** Plugins (AUv2, VST3, CLAP) + Standalone app (`Equaform-8.app` + CoreAudio HAL `Equaform.driver`) in combined DMG via `./scripts/package-distribution-plugins.sh <version>`.
- **Windows (x64):** Plugins only (VST3, CLAP + 15 factory presets) in Inno Setup 6 installer (`scripts/distribution/windows/installer.iss`); no standalone app.

## Test & Lint Status

- **565 tests passed**, 0 failed, 2 ignored (hardware diagnostics) + 2 `compile_fail` doctests in `equaform-engine`
- `cargo clippy --workspace --all-targets -- -D warnings -D clippy::undocumented_unsafe_blocks`: 0 warnings
- `cargo fmt --all -- --check`: clean
- `cargo deny check`: PASS (`advisories ok, bans ok, licenses ok, sources ok`)
- `cargo llvm-cov` core crates (`common`, `dsp`, `analysis`, `engine`): **91.72% lines / 92.83% regions** (>= 90% CI gate)
- Apple `auval -v aufx Eqf8 Eqfm`: `AU VALIDATION SUCCEEDED` (61 DAW params + `CLAP_EXT_LATENCY` bridged to AUv2)
- Criterion stereo SIMD biquad (`equaform-dsp`): -49.5% to -54.2% latency (~2.0x-2.2x throughput, `#![forbid(unsafe_code)]`)

## CI/CD Workflows

- `ci.yml`: 3-OS matrix + `cargo-llvm-cov` >= 90% core DSP/engine gate
- `release.yml`: unified atomic macOS + Windows release
- `nightly.yml`: weekly portable crate health check
- `security.yml`: dependency review and `cargo deny` audit

## Real-Time Safety Guarantees

- `LockFreeTripleBuffer<EqParameters>` for parameter passing (all `unsafe` invariants documented)
- Zero allocations in audio hot path (including stack-only 64-point dual-mode auto-gain)
- Zero panics on channel mismatch in `AudioBuffer` / `EngineLifecycle::process`
- Compile-time Typestate `Engine<Unconfigured | Prepared | Running>` (`equaform-engine/src/typestate.rs`)
- Zero-unsafe stereo SIMD biquad kernels (`equaform-dsp/src/simd.rs`) wired into `BiquadFilter` and `EngineLifecycle`
- Zero `unsafe` in `equaform-cli`; 100% of workspace `unsafe` blocks/impls documented with `// SAFETY:` invariants
- Zero `unwrap`/`expect` in production paths across all 9 crates
- Atomic CPAL error counters, adaptive GUI throttling (60 / 15 / 0 FPS), FFT idle sleep (-90 / -84 dBFS Schmitt trigger)

## Recent Verified Commits

| SHA | Summary |
|-----|---------|
| `c468823` | `chore(release): bump workspace, CLAP, installers, and docs to v1.1.1` (`v1.1.1` tag, pushed) |
| `67b3d5f` | `feat(auto-gain): add Perceptual and musical Anti-Clip auto-gain modes across engine, GUI, and plugins` (pushed) |
| `9521ec2` | `chore(config): add .geminiignore to exclude build artifacts and heavy media` (pushed) |
| `9b34825` | `refactor(plugin): replace 8-param WgpuSurfaceHost::new with WgpuSurfaceConfig` (local) |
| `88b40a7` | `feat(gui): add bounded Undo/Redo stack and replace unsupported toolbar glyphs` (local) |
| `fdd74bc` | `refactor(workspace): split plugin processor and standalone engine client into dedicated modules` (local) |
| `fdcb42c` | `fix(gui): exclude bypass and solo toggles from undo/redo history` (local) |
| `3e9f3e0` | `chore(cleanup): remove stale comment references and write driver pkg atomically` (local) |

## Operational Constraints

- Gate A: Git commit. Gate B: Git push (keep local by default to preserve CI runner quotas).
- 100% English repository; no em-dashes in commit messages; no ticket/phase/audit/ADR references in source code comments.
- Keep `crates/equaform-standalone/resources/EquaformDriver.pkg` unstaged.
