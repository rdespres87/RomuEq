# M2-T004 Evidence: Tests

## Task
M2-T004 — Prototype audio buffer and lifecycle contract.

## Test Suite

All tests are in `crates/romueq-common/src/audio_buffer.rs` under the
`#[cfg(test)]` module.

### Buffer Tests (13 tests)

| Test | Description |
|------|-------------|
| `test_buffer_new_is_empty` | New buffer has zero channels and length |
| `test_buffer_prepare_mono` | Mono buffer prepares correctly |
| `test_buffer_prepare_stereo` | Stereo buffer prepares correctly |
| `test_buffer_prepare_single_sample` | Minimum block size (1 sample) |
| `test_buffer_prepare_large_block` | Large block size near MAX_BLOCK_SIZE |
| `test_buffer_prepare_zero_channels_panics` | Panic on zero channels |
| `test_buffer_prepare_zero_length_panics` | Panic on zero length |
| `test_buffer_prepare_too_many_channels_panics` | Panic on channels > MAX_CHANNELS |
| `test_buffer_prepare_too_large_block_panics` | Panic on length > MAX_BLOCK_SIZE |
| `test_buffer_write_and_read` | Write samples and read them back |
| `test_buffer_zero` | Zeroing all samples |
| `test_buffer_as_input_output_slices` | Slice access for input/output |
| `test_buffer_reuse_on_reprepare` | Allocation reuse on reprepare |

### Lifecycle Tests (16 tests)

| Test | Description |
|------|-------------|
| `test_lifecycle_new_is_created` | Initial state is `Created` |
| `test_lifecycle_prepare_mono` | Mono prepare transitions to `Ready` |
| `test_lifecycle_prepare_stereo` | Stereo prepare transitions to `Ready` |
| `test_lifecycle_invalid_sample_rate_panics` | Panic on zero/negative sample rate |
| `test_lifecycle_invalid_max_block_size_panics` | Panic on zero max block size |
| `test_lifecycle_invalid_channels_panics` | Panic on zero channels |
| `test_lifecycle_process_before_prepare_fails` | Process without prepare returns false |
| `test_lifecycle_process_after_prepare` | Basic process flow |
| `test_lifecycle_process_after_destroy_fails` | Process after destroy returns false |
| `test_lifecycle_process_block_size_1` | Single sample block processing |
| `test_lifecycle_process_block_size_max` | Max block size processing |
| `test_lifecycle_process_block_too_large_fails` | Oversized block returns false |
| `test_lifecycle_process_unequal_input_output_panics` | Channel mismatch panics |
| `test_lifecycle_reprepare` | Reprepare sequence |
| `test_lifecycle_reset` | Reset clears internal state |
| `test_lifecycle_destroy` | Destroy transitions to `Destroyed` |
| `test_lifecycle_variable_block_sizes` | Variable block sizes across process calls |

## Running Tests

```bash
cargo test --workspace --all-features
```

Expected output: 80 tests passed, 0 failed.

## Test Design Principles

1. **Property-based**: Tests verify invariants (e.g. after `prepare`, all
   samples are zero; after `process`, output matches input for identity).
2. **State machine**: Tests verify that invalid state transitions are
   rejected (process before prepare, process after destroy).
3. **Boundary values**: Tests cover edge cases (1 sample, MAX_BLOCK_SIZE,
   MAX_CHANNELS, zero values).
4. **Deterministic**: All tests are deterministic and reproducible. No
   randomness, no timing dependencies.
