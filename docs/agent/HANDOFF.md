# Equaform-8 (RomuEq) - Agent Handoff

Date: 2026-11-26 | Branch: `main` | Workspace: 9-crate Rust

## Current State

- `v1.1.1` ("Reliability & Polish") is published on GitHub Releases (`c468823`).
- `v1.2.0` ("Musical Control & Workflow") is complete locally (maturity audit **9.4/10**, **0 P0s, 0 P1s**):
  - Dual-mode Auto-Gain (`Perceptual` & musical `HeadroomSafe` Anti-Clip, 60 DAW params, `67b3d5f`)
  - `WgpuSurfaceConfig` refactor eliminating the last `#[allow(clippy::too_many_arguments)]` (`9b34825`)
  - Bounded 64-step Undo/Redo stack (`Cmd/Ctrl+Z`, `Cmd/Ctrl+Shift+Z`/`Ctrl+Y`, gesture coalescing, Bypass/Solo excluded) + tofu glyph fixes (`88b40a7`, `fdcb42c`)
  - Monolith splits: `equaform-plugin/src/processor.rs` & `equaform-gui/src/standalone/engine_client.rs` (100% prod files < 1,000 LOC, `fdd74bc`)
  - P2 cleanup: 0 stale comment refs, `~/.equaform` docstrings, atomic PID-scoped `.pkg` write with escaping & readiness polling (`3e9f3e0`)
- Unstaged artifact `crates/equaform-standalone/resources/EquaformDriver.pkg` may appear after `cargo test`; keep unstaged.

## Verified Metrics

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings -D clippy::undocumented_unsafe_blocks` | 0 warnings |
| `cargo test --workspace` | 503 passed, 0 failed, 2 ignored |
| `cargo deny check` | PASS (`advisories/bans/licenses/sources ok`) |
| `cargo llvm-cov` (core crates) | 91.30% lines / 92.41% regions (>= 90% CI gate) |

## Operational Invariants

- **Gate A (commit):** Stage, verify, commit locally. Never auto-push.
- **Gate B (push):** Push only on explicit instruction to preserve CI runner quota.
- 100% English repository; no em-dashes in commit messages; zero ticket/phase/audit/ADR references in comments.
- Keep `EquaformDriver.pkg` unstaged; it is a build artifact, not source.

## Next Logical Actions

1. Push `main` to `origin/main` and push tag `v1.2.0` to trigger `.github/workflows/release.yml` (macOS DMG + Windows Inno Setup EXE).
2. Optional `v1.3.0`: Linear-Phase FIR EQ mode (`Zero Latency | Linear Phase` selector in Master bar + partitioned convolution + `CLAP_EXT_LATENCY` DAW PDC reporting).
