# RomuEq Architecture v2.0

> **Project name status: provisional.** This document uses RomuEq as the current working name.

## 1. Architectural goals

RomuEq is designed as a professional, cross-platform parametric equalizer with a strict real-time audio contract.

The architecture prioritizes:

- deterministic DSP behavior;
- zero allocation in the audio hot path;
- explicit thread ownership;
- sample-accurate automation where the host provides it;
- stable parameter IDs and versioned state;
- separation between DSP, engine, analysis, GUI and host adapters;
- testability without requiring a plugin host.

## 2. MVP processing model

The MVP contains an eight-band minimum-phase EQ. Each band is represented by a fixed-size state structure rather than a trait object in the audio path.

The intended processing chain is:

```text
Host / Standalone input
        |
        v
  Audio buffer contract
        |
        v
  Parameter / automation events
        |
        v
  Real-time engine
        |
        v
  8-band DSP cascade
        |
        v
  Bypass / output gain
        |
        v
  Host / Standalone output
```

## 3. Threading model

### Audio thread

Allowed:

- reading prevalidated parameter state;
- consuming bounded parameter/automation commands;
- coefficient updates using preallocated state;
- DSP processing;
- writing lightweight meter data to a non-blocking transport.

Forbidden:

- heap allocation;
- blocking mutexes;
- filesystem or network I/O;
- logging that can block;
- GUI calls;
- plugin-host calls outside the host API contract;
- intentional panic paths.

### Control / UI thread

Owns:

- GUI state;
- user interaction;
- preset loading;
- state serialization;
- non-real-time validation;
- parameter command production.

### Analysis worker

Owns:

- FFT computation;
- spectrum history;
- frequency-response rendering data;
- spectrogram data.

Analysis must never become a dependency of the audio hot path.

## 4. Parameter transport

The architecture requires a bounded, preallocated producer/consumer protocol.

Two concepts remain separate:

1. control-state changes produced by UI/control code;
2. host automation events carrying a block-relative sample offset.

The exact queue implementation is intentionally not frozen until the concurrency prototype and model checking pass.

## 5. Automation

Automation events must preserve the host-provided sample offset whenever available.

The engine must not silently downgrade sample-accurate events to block-rate changes.

Required evidence:

- dense event tests;
- same-sample ordering tests;
- block-boundary tests;
- variable block-size tests;
- automation/state interaction tests.

## 6. DSP backend

### MVP

- minimum phase;
- biquad-based filters;
- eight bands;
- mono/stereo;
- f32 processing;
- variable host block size.

### Explicitly outside MVP

- linear phase;
- dynamic EQ;
- oversampling;
- multichannel routing;
- sidechain processing;
- Mid/Side processing.

These features must be implemented as explicit processing backends or extensions rather than feature flags that contaminate the MVP hot path.

## 7. Analysis boundary

The spectrum and FFT pipeline is asynchronous from DSP processing.

The audio engine exposes a lightweight analysis source; the analysis worker consumes copied or ring-buffered data according to a separately validated contract.

## 8. State and presets

State is versioned and uses stable parameter identifiers.

A state payload must contain enough information to migrate older versions without relying on current UI defaults.

Presets are user-facing named configurations; plugin state is the host-facing serialization contract. They must not be conflated.

## 9. Bypass and transitions

Bypass behavior must be explicitly defined and tested for clicks and state continuity.

A preferred implementation is a short, deterministic crossfade between processed and bypassed paths. The exact crossfade length must be validated rather than assumed.

## 10. Determinism

The primary promise is functional determinism within documented numerical tolerances.

Bit-exact output is only promised when the compiler, architecture, floating-point behavior and math implementation are sufficiently controlled to make that promise meaningful.

## 11. Plugin boundary

CLAP and VST3 are adapters around the engine, not owners of DSP behavior.

The adapter layer translates:

- host lifecycle;
- audio buffers;
- parameter events;
- state save/restore;
- latency information;
- GUI lifecycle.

No plugin API should leak into `romueq-dsp`.

## 12. Architecture gates

The project is not ready for production DSP implementation until all P0 gates are closed with reproducible evidence.
