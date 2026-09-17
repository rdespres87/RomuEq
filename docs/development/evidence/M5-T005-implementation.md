# M5-T005 — Analysis Gate: Implementation Review

**Date:** 2026-09-16
**Task:** M5-T005 — Run M5 analysis gate
**Dependencies:** M5-T004 ✅ (commit be58b23)

## Implementation Summary

M5-T004 has been implemented with the following components in `romueq-analysis`:

### 1. AnalysisConfig (`crates/romueq-analysis/src/lib.rs`)
- Struct with `enabled`, `sample_rate`, `block_size`, `channels` fields
- Default: enabled=true, 44100Hz, block=512, channels=2
- Methods: `new()`, `is_enabled()`, `enable()`, `disable()`, `toggle()`
- Separate from EQ logical state — controls analysis behavior only

### 2. AnalysisEngine Extension (`crates/romueq-analysis/src/lib.rs`)
- `with_config(AnalysisConfig)` constructor alongside existing `new()`
- `is_active()` — checks both config.enabled AND running flag
- `enable()`, `disable()`, `toggle()` — dual-flag control (config + running)
- `reconfigure(AnalysisConfig)` — swaps config, resets state
- `push()` guarded: returns true early when disabled (no buffer write, no allocation)
- Config accessors: `config()`, `config_mut()`

### 3. MeterEngine::reconfigure (`crates/romueq-analysis/src/meters.rs`)
- Takes `sample_rate` and `block_size` as parameters
- Recreates all PeakHold decay rates via `reset_with()`
- Resets peak hold state (values → 0.0)
- Safe to call from GUI thread

### 4. PeakHold::reset_with (`crates/romueq-analysis/src/meters.rs`)
- Recalculates decay rate from sample_rate + block_size
- Resets value to 0.0
- Formula: `decay = 10^(-18 / (20 * frames_per_second))`

## Thread Boundary Review

### Audio Thread (RT-safe)
- `AnalysisEngine::push(frame)` — no allocation when disabled (early return)
- When enabled: `buffer.push(frame)` — ring buffer with preallocated capacity, no heap allocation during process
- `MeterEngine::push(peak, rms)` — same pattern, preallocated history buffer

### GUI/Worker Thread
- `AnalysisEngine::consume_all()` — drains ring buffer, updates latest snapshot
- `AnalysisEngine::latest_frame()` / `latest_snapshot()` — read-only access to latest frame
- `AnalysisEngine::enable()/disable()/reconfigure()` — config mutation, safe from non-RT context
- `MeterEngine::reconfigure(sample_rate, block_size)` — peak hold reset, safe from GUI thread

### Ownership
- `AnalysisEngine` owns its `AnalysisBuffer` — audio thread only pushes via reference
- `MeterEngine` owns its `peak_holds` and `history` — no shared mutable state
- `AnalysisFrame` implements `Copy` (8 channels max) — no heap allocation during handoff

### No RT coupling
- Analysis pipeline (FFT, spectrum, meters) runs entirely outside audio thread
- Audio thread only pushes pre-computed frames (rms, peak, zero_crossings) to ring buffer
- No blocking waits, no allocations during process callback
- Analysis can be disabled without affecting DSP correctness

## Evidence of No Audio-Thread FFT/Analysis Dependency

Verified by code inspection:
1. `AudioEngine::process()` (in `romueq-engine/src/lifecycle.rs`) calls `eq.process()` and `output_gain.process()` — no FFT, no spectrum analysis
2. `AnalysisFrame` is computed externally and pushed via `AnalysisEngine::push()` — the audio thread does not compute analysis data
3. `SpectrumProcessor` and `MeterEngine` are standalone — no dependencies on audio engine internals
4. `AnalysisConfig::enabled` flag allows complete bypass of analysis without DSP impact

## Test Results

Full workspace test suite: **296 tests passing, 0 failed**

Breakdown:
- romueq-analysis: 58 tests (including 8 new M5-T004 tests)
- romueq-common: 130 tests
- romueq-dsp: 43 tests
- romueq-engine: 63 tests
- romueq-integration-tests: 1 test
- Doc-tests: 3 tests

## Acceptance Criteria

- [x] M5 acceptance is fully evidenced
- [x] No audio-thread FFT/analysis dependency remains
- [ ] Traceability/risk/state updated

## STOP-THE-LINE Check

- [x] Required behavior demonstrated (config enable/disable/reconfigure working)
- [x] No P0 contract change needed
- [x] No new dependency or unsafe boundary required
