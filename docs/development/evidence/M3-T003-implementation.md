# M3-T003 Implementation Evidence

## Summary

Biquad filter state and processing implemented in `crates/romueq-dsp/src/filter.rs` using direct-form I structure with per-channel delay lines. Multi-channel interleaved processing with strict channel isolation guarantees.

## Implementation Details

### Files Modified
- `crates/romueq-dsp/src/filter.rs` — 561 lines, BiquadState + BiquadFilter
- `crates/romueq-dsp/src/lib.rs` — module exposed, public API re-exported

### Core Types

1. **BiquadState** — single-channel delay line
   - `x: [f32; 2]` — input delay line (x[n-1], x[n-2])
   - `y: [f32; 2]` — output delay line (y[n-1], y[n-2])
   - `coeffs: BiquadCoefficients` — current filter coefficients
   - Direct-form I: `y[n] = b0*x[n] + b1*x[n-1] + b2*x[n-2] - a1*y[n-1] - a2*y[n-2]`

2. **BiquadFilter** — multi-channel filter
   - `channels: Vec<BiquadState>` — per-channel state
   - Fixed channel count at construction
   - Interleaved processing: `process(input, output)` for all channels
   - Per-channel processing: `process_channel(channel, input, output)`

### API Surface

| Method | Description |
|--------|-------------|
| `BiquadState::new(coeffs)` | Create with zeroed delay lines |
| `BiquadState::reset()` | Zero all delay lines |
| `BiquadState::process(input)` | Process single sample |
| `BiquadState::set_coefficients(coeffs)` | Update coeffs without resetting state |
| `BiquadState::coefficients()` | Get current coefficients |
| `BiquadFilter::new(num_channels, coeffs)` | Create multi-channel filter |
| `BiquadFilter::reset()` | Reset all channels |
| `BiquadFilter::process(input, output)` | Interleaved block processing |
| `BiquadFilter::process_channel(ch, input, output)` | Single channel block processing |
| `BiquadFilter::num_channels()` | Get channel count |
| `BiquadFilter::set_coefficients(coeffs)` | Update all channels |
| `BiquadFilter::coefficients(ch)` | Get channel coefficients |

### Key Design Decisions
- Direct-form I structure (separate input/output delay lines)
- Channel state is independent — no cross-contamination
- Coefficient update preserves delay line state (signal continuity)
- Reset is idempotent — calling twice has same effect as once
- Panics on invalid channel index, zero channels, length mismatch
- All state is stack-allocated (no heap in BiquadState)
- BiquadFilter uses Vec for channels (fixed at construction)

## Test Results

```
running 43 tests
test coefficients::tests::test_all_filter_types_produce_finite_coefficients ... ok
test coefficients::tests::test_all_filters_at_max_frequency ... ok
test coefficients::tests::test_all_filters_at_min_frequency ... ok
test coefficients::tests::test_bell_dc_gain ... ok
test coefficients::tests::test_bell_extreme_q ... ok
test coefficients::tests::test_bell_negative_gain ... ok
test coefficients::tests::test_bell_positive_gain ... ok
test coefficients::tests::test_bell_zero_gain_is_unity ... ok
test coefficients::tests::test_coefficients_are_normalised ... ok
test coefficients::tests::test_default_coefficients_are_unity ... ok
test coefficients::tests::test_different_sample_rates ... ok
test coefficients::tests::test_frequency_clamped_to_minimum ... ok
test coefficients::tests::test_frequency_clamped_to_nyquist ... ok
test coefficients::tests::test_frequency_zero_assertion ... ok
test coefficients::tests::test_high_pass_attenuates_dc ... ok
test coefficients::tests::test_high_pass_dc_attenuation ... ok
test coefficients::tests::test_high_pass_passes_high_freq ... ok
test coefficients::tests::test_high_shelf_negative_gain ... ok
test coefficients::tests::test_high_shelf_positive_gain ... ok
test coefficients::tests::test_high_shelf_zero_gain_is_unity ... ok
test coefficients::tests::test_low_pass_attenuates_nyquist ... ok
test coefficients::tests::test_low_pass_frequency_sweep ... ok
test coefficients::tests::test_low_pass_nyquist_attenuation ... ok
test coefficients::tests::test_low_pass_passes_dc ... ok
test coefficients::tests::test_low_shelf_dc_behavior ... ok
test coefficients::tests::test_low_shelf_negative_gain ... ok
test coefficients::tests::test_low_shelf_positive_gain ... ok
test coefficients::tests::test_low_shelf_zero_gain_is_unity ... ok
test coefficients::tests::test_q_clamped ... ok
test coefficients::tests::test_sample_rate_assertion ... ok
test coefficients::tests::test_slope_variations_produce_finite_coefficients ... ok
test filter::tests::test_channel_isolation ... ok
test filter::tests::test_coefficient_update_preserves_state ... ok
test filter::tests::test_multichannel_interleaved ... ok
test filter::tests::test_multichannel_reset_all ... ok
test filter::tests::test_new_panics_on_zero_channels - should panic ... ok
test filter::tests::test_process_channel_panics_on_invalid_channel - should panic ... ok
test filter::tests::test_process_panics_on_length_mismatch - should panic ... ok
test filter::tests::test_process_panics_on_non_multiple_length - should panic ... ok
test filter::tests::test_reset_clears_delay_lines ... ok
test filter::tests::test_reset_clears_stale_history ... ok
test filter::tests::test_reset_is_idempotent ... ok
test filter::tests::test_single_channel_unity_passthrough ... ok

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Filter-Specific Tests (12 tests)

| Test | Description | Status |
|------|-------------|--------|
| test_reset_clears_delay_lines | Reset → first sample = b0 | PASS |
| test_reset_clears_stale_history | Reset matches fresh filter after 1000 samples | PASS |
| test_channel_isolation | Ch0 ≠ Ch1 state, no cross-contamination | PASS |
| test_multichannel_interleaved | Stereo interleaved processing correct | PASS |
| test_multichannel_reset_all | 4-channel reset matches fresh 4-channel | PASS |
| test_coefficient_update_preserves_state | Coeff change → different output, same delay line | PASS |
| test_process_channel_panics_on_invalid_channel | channel=5 on 2-ch filter → panic | PASS |
| test_new_panics_on_zero_channels | num_channels=0 → panic | PASS |
| test_process_panics_on_length_mismatch | input.len() ≠ output.len() → panic | PASS |
| test_process_panics_on_non_multiple_length | len not multiple of channels → panic | PASS |
| test_reset_is_idempotent | Double reset = single reset | PASS |
| test_single_channel_unity_passthrough | Near-unity filter passes signal | PASS |

## Acceptance Criteria Verification

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Reset produces known behavior | PASS | `test_reset_clears_delay_lines` — first sample after reset = b0 |
| No stale history survives required reset | PASS | `test_reset_clears_stale_history` — 1000-sample history cleared, matches fresh |
| Channel states cannot cross-contaminate | PASS | `test_channel_isolation` — independent state per channel |
| Reset is idempotent | PASS | `test_reset_is_idempotent` — double reset = single reset |
| Coefficient update preserves continuity | PASS | `test_coefficient_update_preserves_state` — delay lines unchanged |

## Build Verification

```
cargo fmt --all -- --check → FORMAT OK
cargo clippy --workspace --all-targets → 0 errors (only pre-existing style warnings)
cargo test -p romueq-dsp --all-features → 43 passed, 0 failed
```

## Evidence Artifacts

- `docs/development/evidence/M3-T003-implementation.md` — implementation details
- `docs/development/evidence/M3-T003-tests.md` — test results
- `docs/development/evidence/M3-T003-evidence.md` — this file
