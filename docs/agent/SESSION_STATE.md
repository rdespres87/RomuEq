# Equaform-8 (RomuEq) - Session State

**Date:** 2026-11-25
**Version:** v1.1.1 ("Reliability & Polish", ready to tag) — `v1.1.0` live on GitHub Releases
**Maturity Audit:** Post-v1.1.0 audit P0s + preset persistence + i18n + DSP loop hoists resolved

## Release Assets

- `Equaform-8-v1.1.1.dmg` + `.dmg.sha256` (macOS Apple Silicon)
- `Equaform-8-Plugins-Windows-v1.1.1.exe` + `.exe.sha256` (Windows x64)

## Platform Scope

- **macOS (Apple Silicon):** Plugins (AUv2, VST3, CLAP) + Standalone app (`Equaform-8.app` + CoreAudio HAL `Equaform.driver`) in combined DMG via `./scripts/package-distribution-plugins.sh <version>`.
- **Windows (x64):** Plugins only (VST3, CLAP + 15 factory presets) in Inno Setup 6 installer (`scripts/distribution/windows/installer.iss`); no standalone app.

## Test & Lint Status

- 489 tests passed, 0 failed, 2 ignored (hardware diagnostics) + 2 `compile_fail` doctests in `equaform-engine`
- `cargo clippy --workspace --all-targets -- -D warnings -D clippy::undocumented_unsafe_blocks`: 0 warnings
- `cargo fmt --all -- --check`: clean
- `cargo llvm-cov` core crates (`common`, `dsp`, `analysis`, `engine`): 91.57% lines / 92.30% regions (>= 90% CI gate)
- Criterion stereo SIMD biquad (`equaform-dsp`): -49.5% to -54.2% latency (~2.0x-2.2x throughput, `#![forbid(unsafe_code)]`)

## CI/CD Workflows

- `ci.yml`: 3-OS matrix + `cargo-llvm-cov` >= 90% core DSP/engine gate
- `release.yml`: unified atomic macOS + Windows release
- `nightly.yml`: weekly portable crate health check
- `security.yml`: dependency review and `cargo deny` audit

## Real-Time Safety Guarantees

- `LockFreeTripleBuffer<EqParameters>` for parameter passing (all `unsafe` invariants documented)
- Zero allocations in audio hot path
- Zero panics on channel mismatch in `AudioBuffer` / `EngineLifecycle::process`
- Compile-time Typestate `Engine<Unconfigured | Prepared | Running>` (`equaform-engine/src/typestate.rs`)
- Zero-unsafe stereo SIMD biquad kernels (`equaform-dsp/src/simd.rs`) wired into `BiquadFilter` and `EngineLifecycle`
- Zero `unsafe` in `equaform-cli`; 100% of workspace `unsafe` blocks/impls documented with `// SAFETY:` invariants
- Zero `unwrap`/`expect` in `crates/equaform-gui/src/standalone.rs`
- Atomic CPAL error counters
- Adaptive GUI throttling (60 / 15 / 0 FPS)
- FFT idle sleep (-90 / -84 dBFS Schmitt trigger)

## Recent Verified Commits

| SHA | Summary |
|-----|---------|
| `4ca9d21` | `fix(windows): remove invalid createallsubdirs flag on flat preset glob` (`v1.0.0` tag, pushed) |
| `0803449` | `refactor(gui): eliminate unwrap and expect calls in standalone engine client` (local) |
| `1833d42` | `docs(agent): sync session state, tasks, and handoff to v1.0.0 release` (local) |
| `a7cb697` | `feat(gui): sort presets with user presets first and factory presets alphabetically` (local) |
| `df6add5` | `refactor(gui): split audio.rs into 6 cohesive submodules` (local) |
| `2b84fc6` | `feat(engine): add compile-time typestate Engine<S> with sealed states` (local) |
| `25db9a8` | `perf(dsp): add vectorized biquad kernels, wire into engine` (pushed) |
| `0c8fd7b` | `refactor(safety): fix FFI UB and document all SAFETY invariants` (pushed) |
| `1591a38` | `docs(agent): sync session state, tasks, and handoff after maturity plan completion` (pushed) |
| `7cf69a5` | `feat(audition): add monitor-only band solo across all crates` (local) |
| `e4736c9` | `feat(routing): add Mid/Side and per-band stereo routing` (local) |
| `c51e706` | `feat(ab-compare): add A/B slot toggle with dynamic copy` (pushed, `v1.1.0`) |
| `f3b9275` | `fix(analysis): add planar stride-aware AnalysisFrame::compute_planar` (local) |
| `207c550` | `fix(engine): use planar analysis frame and click-free Band Solo crossfade` (local) |
| `c701fd2` | `fix(common,gui): bound text file reads to 16 MiB against TOCTOU` (local) |
| `1738c7c` | `fix(presets): filter ephemeral band solo IDs on save and strip them on load to prevent preset rejection` (local) |
| `de304c4` | `fix(gui): localize Band Solo, A/B compare, and channel routing tooltips` (local) |
| `580d735` | `perf(engine): hoist auto-gain update and Mid/Side solo transforms out of inner loops` (local) |

## Operational Constraints

- Gate A: Git commit. Gate B: Git push (keep local by default to preserve CI runner quotas).
- 100% English repository.
- No em-dashes in commit messages.
- No `ADR-xxx` references in source code comments.
