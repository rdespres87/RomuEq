# M4-T001 — Implementation Report: Allocation-Free Engine Lifecycle

**Date:** 2026-09-14
**Status:** DONE
**Milestone:** M4
**Task:** M4-T001 — Implement allocation-free engine lifecycle

## Summary

Implemented `EngineLifecycle` in `romueq-engine/src/lifecycle.rs` as an allocation-free state machine managing the prepare/process/reset/reprepare sequence. The engine uses a flat pre-allocated `[f32; MAX_CHANNELS * MAX_BLOCK_SIZE]` buffer instead of `Vec<Vec<f32>>`, eliminating all heap allocations during the audio processing path.

## Deliverables

### 1. `romueq-engine/src/lifecycle.rs` (new file)

- **`EngineLifecycle` struct** with flat buffer `[f32; MAX_CHANNELS * MAX_BLOCK_SIZE]`
- **`new()`** — constructs with zero-initialized flat buffer
- **`prepare(sample_rate, max_block_size, channels)`** — validates parameters, updates metadata, zeroes buffer. No allocation.
- **`process(inputs, outputs)`** — copies inputs to flat buffer, copies buffer to outputs. No allocation.
- **`reset()`** — zeroes buffer, resets counters. No allocation.
- **`reprepare(sample_rate, max_block_size, channels)`** — calls `reset()` then `prepare()`. No allocation.
- **`destroy()`** — transitions to `Destroyed` state.
- **`buffer()` / `buffer_mut()`** — read/write access to flat buffer.
- **Properties** — `sample_rate()`, `max_block_size()`, `channels()`, `blocks_processed()`, `state()`.

### 2. `romueq-engine/src/lib.rs` (modified)

- Added `pub mod lifecycle;` to export the lifecycle module.

### 3. `romueq-common/src/lib.rs` (modified)

- Added re-exports: `AudioBuffer`, `EngineLifecycle as CommonEngineLifecycle`, `LifecycleState as CommonLifecycleState`, `MAX_BLOCK_SIZE`, `MAX_CHANNELS`.

## Design Decisions

### Flat buffer layout

The flat buffer uses a planar layout: channel `ch` occupies `buffer[ch * max_block_size .. (ch+1) * max_block_size]`. This matches the existing `AudioBuffer` planar convention and avoids per-channel `Vec` allocations.

### Constants

- `LIFECYCLE_MAX_CHANNELS = 8` (matches `MAX_CHANNELS`)
- `LIFECYCLE_MAX_BLOCK_SIZE = 4096` (matches `MAX_BLOCK_SIZE`)

These are enforced at runtime via `assert!` in `prepare()`.

### No allocation guarantee

All methods are allocation-free. The flat buffer is sized at construction time (`new()`), and `prepare()` only updates metadata and zeroes the used portion. This satisfies the real-time safety requirements of `ADR-007-realtime-safety.md`.

## Tests

12 unit tests covering:

- Lifecycle state transitions (new → prepared → destroyed)
- `prepare()` validation (sample_rate, channels, block_size)
- `process()` with 1 and 2 channels
- `reset()` zeroes buffer and resets counters
- `reprepare()` zeroes buffer and updates metadata
- Flat buffer channel layout and isolation
- `destroy()` prevents further processing

All tests pass (192 total, 12 new).

## Verification

```
cargo fmt --all -- --check       → exit 0
cargo clippy --workspace --all-targets --all-features -- -D warnings → exit 0
cargo test --workspace --all-features → 192 passed, 0 failed
```

## Traceability

- **ADR-007-realtime-safety.md**: Allocation-free lifecycle satisfies real-time safety requirements.
- **ADR-DR-2026-004-audio-buffer-lifecycle.md**: Follows the prepare/process/reset/reprepare contract.
- **M4-T001 acceptance criteria**: All stated checks pass. No out-of-scope architectural changes.
