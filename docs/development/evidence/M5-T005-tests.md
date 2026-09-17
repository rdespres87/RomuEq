# M5-T005 — Analysis Gate: Test Evidence

**Date:** 2026-09-16
**Task:** M5-T005 — Run M5 analysis gate

## Validation Command

```bash
cargo test --workspace --all-features
```

## Results

```
romueq-analysis:    58 passed, 0 failed
romueq-common:     130 passed, 0 failed
romueq-dsp:         43 passed, 0 failed
romueq-engine:      63 passed, 0 failed
romueq-gui:          0 passed, 0 failed (no tests yet)
romueq-integration-tests: 1 passed, 0 failed
romueq-plugin:       0 passed, 0 failed (no tests yet)
romueq-cli:          0 passed, 0 failed (binary)
romueq-standalone:   0 passed, 0 failed (binary)

Doc-tests:           3 passed, 0 failed

Total:              296 passed, 0 failed
```

## M5-Specific Test Coverage

### AnalysisConfig (3 tests)
- `test_analysis_config_default` — default values correct (enabled, 44100Hz, 512 block, 2 channels)
- `test_analysis_config_enable_disable` — enable/disable/toggle state transitions
- `test_analysis_engine_config_accessors` — config getter/setter work correctly

### AnalysisEngine State Management (5 tests)
- `test_analysis_engine_with_config` — custom config constructor works
- `test_analysis_engine_enable_disable_with_frames` — disabled engine doesn't record frames
- `test_analysis_engine_push_disabled_then_reenabled` — push returns true when disabled, buffer stays empty
- `test_analysis_engine_reconfigure` — reconfigure swaps config and keeps engine active
- `test_analysis_engine_new_is_active_by_default` — default construction starts active

### MeterEngine Reconfigure (1 test)
- `test_meter_engine_reconfigure` — reconfiguring sample_rate/block_size resets peak hold values

### Analysis Handoff (6 tests)
- `test_analysis_handoff_simulation` — ring buffer push/pop cycle
- `test_analysis_handoff_stereo_interleaved` — stereo channel layout preserved
- `test_analysis_handoff_drop_policy_deterministic` — drop-new policy verified
- `test_analysis_handoff_no_allocation_in_process` — no heap allocation during handoff
- `test_analysis_handoff_high_load_with_drain` — high-load stress with concurrent drain
- `test_analysis_handoff_stress` — rapid push/pop stress test

### Analysis Frame Computation (7 tests)
- `test_analysis_frame_compute_stereo` — stereo RMS/peak computation
- `test_analysis_frame_compute_mono_sine` — mono sine wave analysis
- `test_analysis_frame_compute_mono_silence` — silence produces zero values
- `test_analysis_frame_compute_block_256` — 256-sample block computation
- `test_analysis_frame_compute_small_block` — small block edge case
- `test_analysis_frame_multi_channel_energy` — multi-channel energy calculation
- `test_analysis_engine_latest_snapshot` — latest frame snapshot consistency

### Spectrum Pipeline (10 tests)
- `test_spectrum_config_default_smoothing` — EMA smoothing defaults
- `test_spectrum_config_reconfigure` — reconfiguring sample rate affects bin frequencies
- `test_spectrum_sine_peak_detection` — known sine frequency peak detection
- `test_spectrum_zero_input` — zero input produces zero spectrum
- `test_spectrum_short_input_zero_padded` — short input zero-padding for FFT
- `test_spectrum_frame_bin_count` — correct FFT bin count
- `test_spectrum_log_bin_count` — log-frequency bin count
- `test_spectrum_window_types` — window function types work correctly
- `test_spectrum_sample_rate_affects_bin_frequency` — sample rate changes bin frequency mapping
- `test_spectrum_ema_smoothing` — EMA smoothing convergence

### Meter Engine (9 tests)
- `test_meter_engine_push_and_latest` — push produces correct latest values
- `test_meter_engine_push_from_analysis` — AnalysisFrame → MeterEngine conversion
- `test_meter_engine_stereo_separate_values` — stereo channels have separate values
- `test_meter_engine_default_is_zero` — default meter values are zero
- `test_meter_engine_reset` — reset clears all state
- `test_meter_engine_buffer_overflow` — buffer overflow handled correctly
- `test_meter_engine_consume_all` — consume_all drains all frames
- `test_meter_engine_history_rotation` — history buffer rotates correctly
- `test_meter_engine_peak_hold_evolution` — peak hold decay over time

### AnalysisBuffer (7 tests)
- `test_analysis_buffer_basic_push_pop` — basic push/pop cycle
- `test_analysis_buffer_drop_new_policy` — drop-new policy on full buffer
- `test_analysis_buffer_is_empty_initially` — new buffer starts empty
- `test_analysis_buffer_lifo_fifo_ordering` — ordering verification
- `test_analysis_buffer_pop_drains` — pop drains correctly
- `test_analysis_buffer_preallocated_no_grow` — no reallocation during operation
- `test_analysis_engine_drops_under_overflow` — drop count tracking
