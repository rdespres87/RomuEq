# M3-T002 Test Evidence

## Test Suite Overview

31 unit tests in `crates/romueq-dsp/src/coefficients.rs` covering all MVP filter types.

## Test Categories

### 1. Coefficient Validity (3 tests)
- `test_coefficients_are_normalised` — verifies `a0 ≈ 1.0` after normalization
- `test_default_coefficients_are_unity` — all coefficients ≈ 1.0 at 0dB
- `test_all_filter_types_produce_finite_coefficients` — no NaN/Inf for any filter type

### 2. Frequency Response (10 tests)
- `test_bell_zero_gain_is_unity` — Bell at 0dB has unity gain
- `test_bell_positive_gain` — Bell at +12dB amplifies center frequency
- `test_bell_negative_gain` — Bell at -12dB attenuates center frequency
- `test_bell_dc_gain` — Bell at DC ≈ unity
- `test_bell_extreme_q` — extreme Q values produce finite coefficients
- `test_high_pass_attenuates_dc` — HPF attenuates DC
- `test_high_pass_passes_high_freq` — HPF passes high frequencies
- `test_low_pass_passes_dc` — LPF passes DC
- `test_low_pass_attenuates_nyquist` — LPF attenuates Nyquist
- `test_high_shelf_zero_gain_is_unity` — HighShelf at 0dB has unity gain

### 3. Shelf Behavior (4 tests)
- `test_low_shelf_dc_behavior` — LowShelf DC gain matches expected
- `test_low_shelf_positive_gain` — LowShelf +12dB boosts DC
- `test_low_shelf_negative_gain` — LowShelf -12dB cuts DC
- `test_high_shelf_positive_gain` — HighShelf +12dB boosts high freq
- `test_high_shelf_negative_gain` — HighShelf -12dB cuts high freq

### 4. Edge Cases (6 tests)
- `test_frequency_clamped_to_minimum` — freq < 1.0 Hz clamped to 1.0
- `test_frequency_clamped_to_nyquist` — freq > Nyquist clamped to Nyquist
- `test_q_clamped` — Q outside [0.001, 100.0] clamped
- `test_sample_rate_assertion` — sample_rate ≤ 0 rejected
- `test_frequency_zero_assertion` — freq = 0 rejected
- `test_different_sample_rates` — coefficients vary correctly with sample rate

### 5. Slope Variations (2 tests)
- `test_slope_variations_produce_finite_coefficients` — all Slope variants finite
- `test_all_filters_at_min_frequency` — all filters finite at 20 Hz
- `test_all_filters_at_max_frequency` — all filters finite at 19 kHz

### 6. Overall Gain (1 test)
- `test_high_shelf_zero_gain_is_unity` — overall gain at 0dB ≈ 1.0

## Verification Method

All tests use `f32` tolerance of `0.01` (1%) for floating-point comparisons.
Shelf filter unity gain verified via overall transfer function `(b0+b1+b2)/(1+a1+a2)`.
