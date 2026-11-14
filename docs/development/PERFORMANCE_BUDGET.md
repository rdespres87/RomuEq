# Performance Budget

## Principle

Performance targets are measured budgets, not guesses. The final reference machine and release thresholds are blocked by `DR-2026-006` until accepted.

## Initial engineering target

The pre-production target is **<5% CPU for stereo 48 kHz processing** on a defined reference machine, measured using a repeatable protocol. This is a provisional engineering target, not a release claim.

## Measurement protocol

Record:

- CPU model and power mode;
- OS version;
- compiler/toolchain version;
- build profile and flags;
- sample rate;
- channel count;
- block-size distribution;
- active bands and filter types;
- automation load;
- denormal handling configuration;
- measurement duration;
- warm-up duration;
- number of instances;
- CPU measurement method.

Run at least:

1. 48 kHz, stereo, 64-sample blocks, 8 active bands.
2. 48 kHz, stereo, 256-sample blocks, 8 active bands.
3. 96 kHz, stereo, 64-sample blocks, 8 active bands.
4. 48 kHz, mono, 64-sample blocks, 8 active bands.
5. Worst-case parameter automation within the supported event rate.
6. Multiple-instance scaling test.

## Reliability budgets

- No unbounded memory growth during processing.
- No allocation in the hot path.
- No lock contention in the hot path.
- No unexplained CPU spikes above the accepted measurement variance.
- No NaN/Inf output for valid inputs and parameters.

## Benchmark acceptance

A benchmark result is valid only when the exact environment and command are recorded. Performance regressions beyond the accepted tolerance require a task or DR; they may not be dismissed as noise without evidence.
