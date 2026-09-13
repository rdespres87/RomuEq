# M3-T003 Evidence Gate

## Task: Implement filter state/reset semantics

**Milestone:** M3
**Status:** DONE
**Commit:** 916192f feat(dsp): implement biquad filter state and processing (M3-T002, M3-T003)
**Dependencies:** M3-T002 (DONE)

## Implementation Summary

Biquad filter state and processing implemented in `crates/romueq-dsp/src/filter.rs` (561 lines) using direct-form I structure with per-channel delay lines. Multi-channel interleaved processing with strict channel isolation guarantees.

### What was implemented

1. **BiquadState** — single-channel delay line
   - `x: [f32; 2]` input delay line, `y: [f32; 2]` output delay line
   - Direct-form I: `y[n] = b0*x[n] + b1*x[n-1] + b2*x[n-2] - a1*y[n-1] - a2*y[n-2]`
   - `reset()` zeroes all delay lines
   - `process(input)` computes single sample
   - `set_coefficients()` updates coeffs without resetting state

2. **BiquadFilter** — multi-channel filter
   - `channels: Vec<BiquadState>` — per-channel state
   - `process(input, output)` — interleaved block processing
   - `process_channel(ch, input, output)` — single channel block processing
   - `reset()` resets all channels
   - `set_coefficients()` updates all channels

3. **12 filter tests** covering reset semantics, channel isolation, multichannel processing, panic conditions, and coefficient update continuity

## Acceptance Criteria

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Reset produces known behavior | PASS | `test_reset_clears_delay_lines` — first sample after reset = b0 |
| No stale history survives required reset | PASS | `test_reset_clears_stale_history` — 1000-sample history cleared |
| Channel states cannot cross-contaminate | PASS | `test_channel_isolation` — independent state per channel |

## Test Coverage

- **Unit tests:** 43 tests (31 coefficients + 12 filter), all passing
- **Doc-test:** 1 test in lib.rs
- **Property tests:** deterministic reset, idempotent reset, coefficient continuity, channel independence

## Build Verification

```
cargo fmt --all -- --check → FORMAT OK
cargo clippy --workspace --all-targets → 0 errors
cargo test -p romueq-dsp --all-features → 43 passed, 0 failed
```

## Files Changed

- `crates/romueq-dsp/src/filter.rs` — 561 lines (new)
- `crates/romueq-dsp/src/lib.rs` — module exposed, public API re-exported

## Evidence Artifacts

- `docs/development/evidence/M3-T003-implementation.md` — implementation details
- `docs/development/evidence/M3-T003-tests.md` — test results
- `docs/development/evidence/M3-T003-evidence.md` — this file

## No Stop-the-Line Conditions

- No P0 contract changes required
- No new dependencies or unsafe boundaries
- All acceptance criteria verified
- All tests passing
