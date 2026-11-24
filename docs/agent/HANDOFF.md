# Equaform-8 (RomuEq) - Agent Handoff

Date: 2026-11-24 | Branch: `main` | Workspace: 9-crate Rust

## Current State

- Tag `v1.0.0` published at `4ca9d21`; all 4 GitHub Release assets live (macOS `.dmg` + `.sha256`, Windows `.exe` + `.sha256`).
- Local commits ahead of `origin/main` (not pushed; see Gate B below).
- Unstaged artifact `crates/equaform-standalone/resources/EquaformDriver.pkg` may appear after `cargo test`; keep unstaged or restore with `git checkout --`.
- All P0/P1 maturity items from the v1.0.0 audit (8.2/10) are complete.

## Verified Metrics

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings |
| `cargo test --workspace` | 448 passed, 0 failed, 2 ignored |

## Recent Commits

| SHA | Message | Status |
|---|---|---|
| `0803449` | `refactor(gui): eliminate unwrap and expect calls in standalone engine client` | Local, unpushed |
| `4ca9d21` | `fix(windows): remove invalid createallsubdirs flag on flat preset glob` (`v1.0.0` tag) | Pushed, tagged |

## Operational Invariants

- **Gate A (commit):** Stage, verify, commit locally. Never auto-push.
- **Gate B (push):** Push only on explicit instruction to preserve CI runner quota.
- 100% English repository; no em-dashes in commit messages.
- No `ADR-xxx` references in source code comments.
- Keep `EquaformDriver.pkg` unstaged; it is a build artifact, not source.

## Next Logical Actions (P2 Roadmap Toward 9.5/10)

1. Modularize oversized files: `audio.rs` (1701 L), `state.rs` (1342 L), `parameter.rs` (1285 L), `equaform-plugin/src/lib.rs` (1204 L), `standalone.rs` (1176 L).
2. Audit and add `// SAFETY:` comments across all `unsafe` blocks (FFI, HAL, SIMD).
3. Evaluate Typestate `Engine<State>` pattern, SIMD biquad vectorization, and DSP Pro features (Mid/Side, Band Solo, Linear-phase EQ).
