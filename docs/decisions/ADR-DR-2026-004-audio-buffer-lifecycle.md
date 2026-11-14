# ADR-DR-2026-004: Audio Buffer and Lifecycle Contract

## Status
Accepted

## Context
Equaform must process audio in real-time with deterministic memory behaviour.
The engine needs a well-defined audio buffer representation and a strict
lifecycle contract between the host and the plugin. Prior to this decision,
the codebase had no audio buffer type and no lifecycle management —
`equaform-engine` and `equaform-plugin` were both stubs.

We need to decide:
1. How audio data is laid out in memory (planar vs interleaved).
2. What the lifecycle state machine looks like.
3. How `prepare`, `process`, `reset`, and `reprepare` interact.
4. What the invalid-host behaviour contract is (panic vs graceful degradation).

## Decision

### Buffer representation: planar f32

We use a **planar** layout: one contiguous `Vec<f32>` per channel, stored in
a `Vec<Vec<f32>>`. For stereo 256 samples:

```
channel 0: [f32; 256]  // samples 0..256
channel 1: [f32; 256]  // samples 0..256
```

This matches CLAP and VST3 conventions and avoids interleaved copies.
The `AudioBuffer` type exposes `channel()` and `channel_mut()` for single-
channel access, and `as_input_slices()` / `as_output_slices()` for bulk
access.

### Lifecycle state machine

```
Created ──prepare()──→ Ready
Ready   ──process()──→ Ready
Ready   ──reset()────→ Ready
Ready   ──prepare()──→ Ready  (reprepare)
Ready   ──destroy()──→ Destroyed
Destroyed ──destroy()──→ Destroyed  (idempotent)
```

- `prepare(sample_rate, max_block_size, channels)` — allocates/reallocates
  internal buffers, zeroes all samples. Called outside the RT path.
- `process(inputs, outputs)` — copies inputs to internal buffer, runs
  identity pass-through (DSP wired in M3), copies to outputs. RT-safe:
  no allocation, no blocking, no syscalls.
- `reset()` — clears internal state (delays, filters, meters) without
  changing sample rate or channel configuration.
- `destroy()` — transitions to `Destroyed`, idempotent.

### Invalid-host behaviour: panic

When the host violates the contract (e.g. `inputs.len() != outputs.len()`,
zero sample rate, channel count out of range), the engine **panics**.
This is intentional for the MVP: it catches bugs early during development
and testing. In production, the host SDK layer (JUCE/VST3/AU) will wrap
these calls in `catch_unwind` to prevent a plugin panic from crashing the
host.

### Constants

- `MAX_CHANNELS = 8` — supports mono through octophonic.
- `MAX_BLOCK_SIZE = 8192` — covers all standard DAW block sizes.

## Consequences

### Positive
- **Explicit contract**: `prepare`, `process`, `reset`, `destroy` are
  clearly defined with preconditions and postconditions.
- **RT-safe processing**: `process` has no allocation, no branching on
  buffer shape, no syscalls.
- **Deterministic invalid-host behaviour**: panics are predictable and
  testable.
- **Reusable allocations**: `prepare` reuses existing `Vec` capacity when
  possible, reducing reallocation overhead.

### Negative
- **Panic on invalid host**: requires the host SDK layer to implement
  `catch_unwind` wrappers before shipping to end users.
- **Planar layout**: some host APIs (e.g. VST2) use interleaved buffers,
  requiring a conversion step at the boundary.
- **`Vec<Vec<f32>>` indirection**: two levels of pointer chasing. For the
  MVP this is acceptable; a flat `Vec<f32>` with stride-based indexing
  could be a future optimisation.

## References
- `crates/equaform-common/src/audio_buffer.rs` — `AudioBuffer` and
  `EngineLifecycle` implementations
- `tasks.json` — M2-T004
- CLAP spec: https://github.com/free-audio/clap/blob/master/docs/plugin.md#audio-ports
