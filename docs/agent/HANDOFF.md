# Equaform-8 (RomuEq) - Agent Handoff

Date: 2026-11-25 | Branch: `main` | Workspace: 9-crate Rust

## Current State

- `v1.1.0` ("Pro Mixing") is published on GitHub Releases (`b032ce6`).
- `v1.1.1` ("Reliability & Polish") is complete locally (6 atomic fix/perf commits + version/docs bump commit):
  - Planar stride-aware `AnalysisFrame::compute_planar` fixing L/R stereo VU-metering (`f3b9275`, `207c550`)
  - Click-free equal-power Band Solo crossfade (`SoloTransitionState`, zero audio-thread allocs) (`207c550`)
  - TOCTOU-safe 16 MiB `read_to_string_bounded` across preset, state, and i18n loaders (`c701fd2`)
  - Ephemeral Band Solo ID filtering (`106..=806`) on preset save and stripping on load/import (`1738c7c`)
  - Full 4-language (EN/FR/DE/ES) localization of Band Solo, A/B Compare, and Channel Routing tooltips (`de304c4`)
  - DSP loop hoisting (`update_auto_gain_db` + Mid/Side solo transforms) and dead code cleanup (`580d735`)
- Unstaged artifact `crates/equaform-standalone/resources/EquaformDriver.pkg` may appear after `cargo test`; keep unstaged.

## Verified Metrics

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings -D clippy::undocumented_unsafe_blocks` | 0 warnings |
| `cargo test --workspace` | 489 passed, 0 failed, 2 ignored |
| `cargo deny check` | PASS (`advisories/bans/licenses/sources ok`) |
| `cargo llvm-cov` (core crates) | 91.57% lines / 92.30% regions (>= 90% CI gate) |

## Recent Commits

| SHA | Message | Status |
|---|---|---|
| `580d735` | `perf(engine): hoist auto-gain update and Mid/Side solo transforms out of inner loops` | Local |
| `de304c4` | `fix(gui): localize Band Solo, A/B compare, and channel routing tooltips` | Local |
| `1738c7c` | `fix(presets): filter ephemeral band solo IDs on save and strip them on load to prevent preset rejection` | Local |
| `c701fd2` | `fix(common,gui): bound text file reads to 16 MiB against TOCTOU` | Local |
| `207c550` | `fix(engine): use planar analysis frame and click-free Band Solo crossfade` | Local |
| `f3b9275` | `fix(analysis): add planar stride-aware AnalysisFrame::compute_planar` | Local |

## Operational Invariants

- **Gate A (commit):** Stage, verify, commit locally. Never auto-push.
- **Gate B (push):** Push only on explicit instruction to preserve CI runner quota.
- 100% English repository; no em-dashes in commit messages.
- Keep `EquaformDriver.pkg` unstaged; it is a build artifact, not source.

## Next Logical Actions

1. Push `main` to `origin/main` to validate 3-OS CI, then tag `v1.1.1` to trigger `.github/workflows/release.yml` (macOS DMG + Windows Inno Setup EXE).
2. `v1.2.0` candidates: `UndoStack<EqParameters>` (`Cmd/Ctrl+Z`), `WgpuSurfaceConfig`, >1000 LOC file splits (`equaform-plugin/src/lib.rs`, `standalone.rs`), and Linear-Phase EQ mode.
