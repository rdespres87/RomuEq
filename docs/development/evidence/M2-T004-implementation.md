# M2-T004 Evidence: Audio Buffer and Lifecycle Contract

## Task
Prototype audio buffer and lifecycle contract to prove that the buffer
representation and lifecycle management are explicit, safe, and testable
before production DSP/engine code.

## Acceptance Criteria

### ✅ Buffer contract is explicit
- `AudioBuffer` uses planar f32 layout: one `Vec<f32>` per channel
- `MAX_CHANNELS = 8`, `MAX_BLOCK_SIZE = 8192`
- `prepare(channels, length)` allocates and zeroes all samples
- `channel(ch)` / `channel_mut(ch)` provide single-channel access
- `as_input_slices()` / `as_output_slices()` provide bulk access
- Buffer is pre-allocated and reused across blocks

### ✅ Invalid host behaviour is safe and deterministic
- Zero or negative sample rate → panic
- Zero or excessive channels → panic
- Zero or excessive block size → panic
- `inputs.len() != outputs.len()` in `process()` → panic
- All panics have descriptive messages
- Host SDK layer will wrap calls in `catch_unwind` for production

### ✅ prepare/reprepare/reset semantics are testable
- `prepare()` allocates and zeroes all samples
- `reprepare()` reuses existing allocations, zeroes all samples
- `reset()` clears internal state without changing configuration
- `destroy()` transitions to `Destroyed`, idempotent
- `process()` before `prepare()` → returns `false`
- `process()` after `destroy()` → returns `false`
- `process()` with block size > max → returns `false`

## Files Modified

### `crates/romueq-common/src/audio_buffer.rs` (new)
- `AudioBuffer` — planar f32 mono/stereo buffer (1-8 channels, max 8192 samples)
  - `new()`, `prepare()`, `channel()`, `channel_mut()`
  - `as_input_slices()`, `as_output_slices()`, `zero()`
  - `channels()`, `length()`, `is_prepared()`
- `EngineLifecycle` — lifecycle state machine
  - `new()`, `prepare()`, `process()`, `reset()`, `destroy()`, `reprepare()`
  - `sample_rate()`, `blocks_processed()`, `state()`
  - `is_created()`, `is_ready()`, `is_destroyed()`
- 29 tests covering buffer operations, lifecycle sequences, and invalid inputs

### `crates/romueq-common/src/lib.rs` (modified)
- Added `pub mod audio_buffer;`

## Test Results

```
romueq-common: 62 tests passed (29 audio_buffer tests)
romueq-engine: 17 tests passed
romueq-plugin: 0 tests (re-exports only)
romueq-integration-tests: 1 test passed
Total: 80 tests passed, 0 failed
```

### Audio buffer test coverage
- `test_buffer_new_is_empty` — initial state
- `test_buffer_prepare_mono` — mono preparation
- `test_buffer_prepare_stereo` — stereo preparation
- `test_buffer_prepare_single_sample` — minimum block size
- `test_buffer_prepare_large_block` — large block size
- `test_buffer_prepare_zero_channels_panics` — invalid channels
- `test_buffer_prepare_zero_length_panics` — invalid length
- `test_buffer_prepare_too_many_channels_panics` — exceeds MAX_CHANNELS
- `test_buffer_prepare_too_large_block_panics` — exceeds MAX_BLOCK_SIZE
- `test_buffer_write_and_read` — basic read/write
- `test_buffer_zero` — zeroing all samples
- `test_buffer_as_input_output_slices` — slice access
- `test_buffer_reuse_on_reprepare` — allocation reuse
- `test_lifecycle_new_is_created` — initial state
- `test_lifecycle_prepare_mono` — mono prepare
- `test_lifecycle_prepare_stereo` — stereo prepare
- `test_lifecycle_invalid_sample_rate_panics` — invalid sample rate
- `test_lifecycle_invalid_max_block_size_panics` — invalid max block size
- `test_lifecycle_invalid_channels_panics` — invalid channels
- `test_lifecycle_process_before_prepare_fails` — process without prepare
- `test_lifecycle_process_after_prepare` — basic process
- `test_lifecycle_process_after_destroy_fails` — process after destroy
- `test_lifecycle_process_block_size_1` — single sample block
- `test_lifecycle_process_block_size_max` — max block size
- `test_lifecycle_process_block_too_large_fails` — oversized block
- `test_lifecycle_process_unequal_input_output_panics` — channel mismatch
- `test_lifecycle_reprepare` — reprepare sequence
- `test_lifecycle_reset` — reset clears state
- `test_lifecycle_destroy` — destroy transitions
- `test_lifecycle_variable_block_sizes` — variable block sizes

## Build Verification

```
cargo check --workspace     → exit 0
cargo fmt --all -- --check  → exit 0
cargo clippy --workspace    → exit 0
cargo test --workspace      → exit 0 (80 tests)
```

## Dependency Graph Verification

```
romueq-common (audio_buffer module)
    ↑
    └── romueq-engine
            ↑
            ├── romueq-cli
            ├── romueq-standalone
            └── romueq-integration-tests
```

No new dependencies introduced. `audio_buffer.rs` lives in `romueq-common`
and is self-contained.

## Conclusion

M2-T004 is complete. The audio buffer and lifecycle contract are implemented
with:
- Planar f32 buffer with explicit preconditions
- Strict lifecycle state machine (created → ready → processing → destroyed)
- Panic on invalid host behaviour (deterministic, testable)
- Full test coverage (29 audio_buffer tests)
- Zero warnings from clippy
- ADR-DR-2026-004 documenting the design decisions
