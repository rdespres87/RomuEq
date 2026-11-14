# Equaform MVP Product Contract

**Status:** Accepted for engineering baseline; public brand remains provisional pending formal clearance.
**Decision authority:** Product owner approval recorded 2026-09-03.

## Product identity

- Public working product name: **Equaform**.
- Tagline: **Precision Audio Processing**.
- The name may be used inside the repository and engineering artifacts.
- No commercial/public release under the name is permitted until formal trademark/name clearance is completed.

## MVP scope

The MVP is an 8-band minimum-phase parametric equalizer with:

- mono and stereo processing;
- f32 processing;
- variable host block sizes;
- sample-accurate automation where the host provides event offsets;
- bypass and output gain controls;
- optional auto-gain according to the defined parameter contract;
- meters and non-real-time spectrum analysis;
- versioned presets and plugin state;
- standalone application;
- CLI/offline renderer;
- CLAP and VST3 adapters after adapter validation.

## Explicitly deferred

Dynamic EQ, linear-phase processing, oversampling, Mid/Side mode, multichannel processing, sidechain processing, WASM, AU/AUv3, and advanced modulation are not MVP requirements.

## Processing contract

The audio hot path must remain allocation-free, non-blocking, I/O-free, syscall-free and GUI-free. Core processing remains host-independent and safe Rust unless an approved adapter boundary requires narrowly isolated unsafe code.

## Architecture decisions accepted in principle

- Parameter transport: bounded preallocated SPSC control-command path plus a separate host automation event stream preserving block-relative sample offsets.
- Plugin architecture: isolated CLAP and VST3 adapters; do not couple the engine to a third-party framework before its maintenance/licensing/ABI evidence is accepted.
- GUI architecture: remains deliberately unfrozen until embedding, standalone, HiDPI, lifecycle, threading, input and packaging prototypes pass.
- Performance: the existing CPU target remains provisional until the benchmark protocol and reference machine are frozen.

## Remaining M0 conditions

The accepted engineering baseline does not claim legal trademark clearance. Formal name/trademark clearance is a release gate before public/commercial distribution. Exact DAW/version test entries are a pre-M7 compatibility deliverable; the M0 platform/format baseline is already frozen.
