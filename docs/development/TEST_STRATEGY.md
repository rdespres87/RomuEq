# Test Strategy

## Test pyramid

1. **Unit tests:** pure functions, parameter validation, serialization, coefficient calculations.
2. **Property tests:** parameter ranges, state round trips, numerical invariants, random automation sequences.
3. **Concurrency tests:** queue ordering, boundedness, overflow policy, producer/consumer stress and model checking where practical.
4. **Audio reference tests:** impulse, sine, swept sine, white/pink-noise fixtures and analytical/reference coefficient comparisons.
5. **Integration tests:** engine lifecycle, block-size changes, sample-rate changes, automation offsets, state and presets.
6. **Host tests:** plugin scan, instantiate, GUI, automation, state, bypass, offline rendering, multiple instances and hostile lifecycle sequences.
7. **Release tests:** package integrity, reproducibility, signing/notarization where applicable, clean-machine smoke tests.

## Required invariants

### Real-time

No allocation, blocking synchronization, I/O, syscall, GUI operation or logging in the audio hot path unless explicitly justified by an API contract and verified.

### Parameter transport

All valid commands are ordered; overflow behavior is deterministic; host automation preserves block-relative offsets; stale or malformed commands cannot corrupt state.

### DSP

Coefficient generation is finite and bounded for all accepted parameters. Filter stability is verified. Bypass and reset behavior are deterministic within documented tolerances.

### State

Save/load is lossless for the supported schema within documented serialization constraints. Unknown future fields are handled according to the schema policy. Version migration is tested.

## Code Coverage & TDD Gates

Development strictly follows the Spec-Driven TDD methodology detailed in `docs/development/TDD_METHODOLOGY.md`.
Code coverage is tracked via `cargo-llvm-cov` and enforced in CI:
- **Core Real-Time & DSP Crates** (`equaform-dsp`, `equaform-engine`, `equaform-analysis`, `equaform-common`): **$\ge 90\%$ line coverage required**.
- **Workspace Overall**: Continuously reported in GitHub Actions step summaries.

## Allocation testing

Use an allocation-counting test harness for engine processing and prove zero allocations over representative block sequences. The harness must distinguish setup-time allocations from `process()` allocations.

## Fuzzing/property testing

Fuzz:

- state decoding;
- parameter decoding;
- automation event ordering;
- queue commands;
- random valid/invalid filter parameters;
- block-size/sample-rate transitions.

A crash, panic, undefined state or unbounded resource use is a P0 failure where it affects a release path.

## Golden fixtures

Reference fixtures must be versioned and include generation parameters. Changing a fixture requires an ADR or explicit test-vector update task with numerical rationale.

## Flaky-test policy

A flaky test is a failure. It may not be quarantined without a P1/P0 risk entry, owner, reproducibility notes and a bounded remediation task.
