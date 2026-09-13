# M3-T001 Tests Evidence

## Test Suite: `cargo test -p romueq-common --all-features`

**Result:** 104 tests passed, 0 failed

## Parameter Module Tests (30 tests)

| Test | Description | Status |
|------|-------------|--------|
| test_param_id_category_global | ParamId(1) → Global | PASS |
| test_param_id_category_band | ParamId(100) → Band | PASS |
| test_param_id_category_reserved | ParamId(900) → Reserved | PASS |
| test_param_id_category_future | ParamId(1000) → Future | PASS |
| test_param_id_category_invalid | ParamId(0) → Invalid | PASS |
| test_param_id_band_index | ParamId(150) → band 0, ParamId(250) → band 1 | PASS |
| test_param_id_band_offset | ParamId(103) → offset 3 | PASS |
| test_param_id_display | Display format "ParamId(1)" | PASS |
| test_parameter_info_global | parameter_info(1) returns BYPASS info | PASS |
| test_parameter_info_band | parameter_info(100) returns band info | PASS |
| test_parameter_info_invalid | parameter_info(0) returns None | PASS |
| test_parameter_info_reserved | parameter_info(900) returns None | PASS |
| test_parameter_info_future | parameter_info(1000) returns None | PASS |
| test_parameter_info_all_known_ids | All IDs 1-5, 100-105, 200-205, ... return Some | PASS |
| test_parameter_info_all_unknown_ids | IDs 0, 999, 1000 return None | PASS |
| test_range_clamp_in_range | clamp(0.5, 0.0, 1.0) → 0.5 | PASS |
| test_range_clamp_below_min | clamp(-0.5, 0.0, 1.0) → 0.0 | PASS |
| test_range_clamp_above_max | clamp(1.5, 0.0, 1.0) → 1.0 | PASS |
| test_range_validate | validate(0.5) → true, validate(1.5) → false | PASS |
| test_validate_value_valid | validate_value(ParamId(1), 0.0) → Ok | PASS |
| test_validate_value_above_max | validate_value(ParamId(1), 2.0) → Err | PASS |
| test_validate_value_below_min | validate_value(ParamId(1), -1.0) → Err | PASS |
| test_validate_value_invalid_id | validate_value(ParamId(0), 0.0) → Err | PASS |
| test_validate_value_boundary_global | Boundary values at min/max | PASS |
| test_validate_value_boundary_band | Boundary values for band params | PASS |
| test_clamp_value_in_range | clamp preserves valid values | PASS |
| test_clamp_value_below_min | clamp(-1.0, 0.0, 1.0) → 0.0 | PASS |
| test_clamp_value_above_max | clamp(2.0, 0.0, 1.0) → 1.0 | PASS |
| test_clamp_value_unknown_id | clamp for unknown ID returns default | PASS |
| test_clamp_preserves_valid_values | clamp doesn't modify in-range values | PASS |
| test_invalid_id_validation | Invalid ID cannot create illegal state | PASS |
| test_global_parameters_default | Default global params match contract | PASS |
| test_global_parameter_boundaries | Global param ID boundaries | PASS |
| test_band_parameters_default | Default band params match contract | PASS |
| test_band_parameter_boundaries_all_bands | All 8 bands have valid param boundaries | PASS |
| test_eq_parameters_default | EQ params default correctly | PASS |
| test_eq_parameters_copy | Eq params copy correctly | PASS |
| test_filter_type_roundtrip | FilterType Display ↔ FromStr roundtrip | PASS |
| test_slope_roundtrip | Slope Display ↔ FromStr roundtrip | PASS |
| test_processing_mode_roundtrip | ProcessingMode Display ↔ FromStr roundtrip | PASS |
| test_stereo_mode_roundtrip | StereoMode Display ↔ FromStr roundtrip | PASS |
| test_parameter_error_display | ParameterError Display format | PASS |

## Automation Module Tests (13 tests)

| Test | Description | Status |
|------|-------------|--------|
| test_event_is_valid_for_block | offset < block_length → true | PASS |
| test_event_offset_at_block_length_is_invalid | offset == block_length → false | PASS |
| test_sort_key_ordering | (offset, sequence) ordering | PASS |
| test_batch_drops_out_of_range_offsets | offsets >= block_length dropped | PASS |
| test_batch_empty_block_length_rejects_all | block_length=0 → empty batch | PASS |
| test_batch_sorts_by_offset_then_sequence | Sorted by (offset, sequence) | PASS |
| test_batch_deduplicates_same_param_same_offset | Keeps highest sequence | PASS |
| test_batch_keeps_different_params_same_offset | Different params kept | PASS |
| test_batch_dedup_different_offset_same_param | Different offsets kept | PASS |
| test_batch_deterministic_ordering | Same input order → same batch | PASS |
| test_batch_large_event_count | 10000 events → ≤2048 after dedup | PASS |
| test_stream_accumulates_and_finalizes | Push → finalize → batch | PASS |
| test_stream_drops_out_of_range_on_finalize | Out-of-range dropped during finalize | PASS |
| test_value_provider_events_for_param | Filter events by param_id | PASS |
| test_value_provider_linear_interpolation | Linear interp between events | PASS |
| test_value_provider_hold_mode | Hold mode returns last value | PASS |
| test_value_provider_no_events_returns_last_known | No events → last_known | PASS |
| test_malformed_offsets_cannot_corrupt_state | Malformed offsets rejected | PASS |
| test_random_event_sequences_produce_deterministic_batches | Random order → deterministic batch | PASS |

## Audio Buffer Tests (15 tests)

| Test | Description | Status |
|------|-------------|--------|
| test_buffer_new_is_empty | New buffer: 0 channels, 0 length | PASS |
| test_buffer_prepare_mono | Mono 256 samples | PASS |
| test_buffer_prepare_stereo | Stereo 512 samples | PASS |
| test_buffer_prepare_single_sample | 1 sample edge case | PASS |
| test_buffer_prepare_large_block | MAX_BLOCK_SIZE (8192) | PASS |
| test_buffer_prepare_zero_channels_panics | channels=0 → panic | PASS |
| test_buffer_prepare_too_many_channels_panics | channels=9 → panic | PASS |
| test_buffer_prepare_zero_length_panics | length=0 → panic | PASS |
| test_buffer_prepare_too_large_block_panics | length>8192 → panic | PASS |
| test_buffer_write_and_read | Write and read back values | PASS |
| test_buffer_as_input_output_slices | as_input_slices / as_output_slices | PASS |
| test_buffer_zero | Zero all channels | PASS |
| test_buffer_reuse_on_reprepare | Reprepare reuses allocations | PASS |
| test_lifecycle_new_is_created | New lifecycle → Created | PASS |
| test_lifecycle_prepare_mono | Prepare mono | PASS |
| test_lifecycle_prepare_stereo | Prepare stereo | PASS |
| test_lifecycle_process_after_prepare | Process after prepare | PASS |
| test_lifecycle_process_before_prepare_fails | Process before prepare → false | PASS |
| test_lifecycle_process_after_destroy_fails | Process after destroy → false | PASS |
| test_lifecycle_process_block_size_1 | Block size 1 | PASS |
| test_lifecycle_process_block_size_max | Block size MAX_BLOCK_SIZE | PASS |
| test_lifecycle_process_block_too_large_fails | Block too large → false | PASS |
| test_lifecycle_process_unequal_input_output_panics | Unequal len → panic | PASS |
| test_lifecycle_reset | Reset clears state | PASS |
| test_lifecycle_reprepare | Reprepare works | PASS |
| test_lifecycle_variable_block_sizes | Variable block sizes | PASS |
| test_lifecycle_destroy | Destroy → Created | PASS |
| test_lifecycle_invalid_sample_rate_panics | sample_rate=0 → panic | PASS |
| test_lifecycle_invalid_max_block_size_panics | max_block_size=0 → panic | PASS |
| test_lifecycle_invalid_channels_panics | channels=0 → panic | PASS |

## Plugin Tests (5 tests)

| Test | Description | Status |
|------|-------------|--------|
| test_adapter_lifecycle | Full lifecycle Created→Ready→Processing→Ready→Deactivated→Ready→Destroyed | PASS |
| test_adapter_init_fails_if_not_created | Double init fails | PASS |
| test_adapter_process_fails_if_not_ready | Process before init → no-op | PASS |
| test_adapter_destroy_is_idempotent | Double destroy → no-op | PASS |
| test_adapter_plugin_access | plugin() returns correct id/name | PASS |

## Ring Buffer Tests (5 tests)

| Test | Description | Status |
|------|-------------|--------|
| test_ring_basic_enqueue_dequeue | Basic enqueue/dequeue | PASS |
| test_ring_wraparound | Wraparound works | PASS |
| test_ring_full_drop_new | Full → drop new | PASS |
| test_ring_full_overwrite_oldest | Full → overwrite oldest | PASS |
| test_ring_setall_command | Setall command | PASS |

## Double Buffer Tests (3 tests)

| Test | Description | Status |
|------|-------------|--------|
| test_double_buffer_basic | Basic produce/consume | PASS |
| test_double_buffer_interleaved_produce_consume | Interleaved operations | PASS |
| test_double_buffer_swap_on_full | Swap when full | PASS |

## Property Tests

- **Deterministic batching**: Random event orderings produce identical batches (test_random_event_sequences_produce_deterministic_batches)
- **Malformed offset safety**: Out-of-range offsets cannot corrupt state (test_malformed_offsets_cannot_corrupt_state)
- **Large-scale dedup**: 10000 events with 8 params × 256 offsets → ≤2048 unique pairs (test_batch_large_event_count)

## Build Verification

```
cargo fmt --all -- --check → FORMAT OK
cargo clippy --workspace --all-targets → 0 errors
cargo test -p romueq-common --all-features → 104 passed, 0 failed
```
