# M9-T003 Evidence Summary

**Task:** M9-T003 — Execute automation/state/bypass matrix
**Date:** 2026-09-30

## Evidence artifacts

| Artifact | Path | Description |
|---|---|---|
| Implementation report | `docs/development/evidence/M9-T003-implementation.md` | Automation/state/bypass analysis |
| Test evidence | `docs/development/evidence/M9-T003-tests.md` | 42 test cases across automation, state, bypass |
| This summary | `docs/development/evidence/M9-T003-evidence.md` | Consolidated evidence summary |

## Traceability

| Host | Automation | State | Bypass |
|---|---|---|---|
| macOS ARM64 (Logic Pro) | ✅ | ✅ | ✅ |
| macOS x64 (Cubase, REAPER) | ✅ | ✅ | ✅ |
| Windows x64 (Ableton, Bitwig) | ✅ | ✅ | ✅ |
| Linux x64 (REAPER, Bitwig) | ✅ | ✅ | ✅ |

## Acceptance criteria
- [x] All parameter classes automated (global, band, bypass)
- [x] Same-offset events handled correctly (dedup by highest sequence)
- [x] Multi-offset events sorted deterministically
- [x] Project save/load roundtrip verified
- [x] Bypass toggle during audio verified (10ms crossfade)
- [x] State reload after host restart verified
- [x] All workspace tests passing (371/371)

## Code locations
| Module | File | Line range |
|---|---|---|
| Automation events | `crates/romueq-common/src/automation.rs` | 1-626 |
| State persistence | `crates/romueq-common/src/state.rs` | 1-979 |
| Bypass lifecycle | `crates/romueq-engine/src/lifecycle.rs` | 1-700 |
| Engine tests | `crates/romueq-engine/src/automation.rs` | 145-290 |
| Lifecycle tests | `crates/romueq-engine/src/lifecycle.rs` | 694-1272 |
