# RomuEq — PROVISIONAL PROJECT NAME

> **Naming status: NOT CLEARED.** RomuEq is the current candidate name only. Public release should wait for final name clearance.

# RomuEq — Provisional Project Name

> **Naming status: NOT CLEARED.** RomuEq is currently used as a working name only. Public release, trademark filing, domain acquisition, and repository publication must wait for final name clearance.

Professional, cross-platform parametric equalizer.

## Project status

**Pre-development / architecture validation.**

No production DSP implementation should begin until the P0 architecture gates are closed.

## MVP targets

- Standalone desktop application
- CLI / offline renderer
- CLAP
- VST3
- 8-band minimum-phase EQ
- Mono / stereo
- Sample-accurate automation where supported by the host
- Meters and spectrum analysis
- Presets and versioned state
- Cross-platform builds: macOS, Windows, Linux

## Future scope

- AU / AUv3
- Dynamic EQ
- Linear-phase processing
- Oversampling
- Mid/Side processing
- WASM
- Multichannel processing

## Repository layout

```text
crates/
  romuq-common/             Shared types, IDs, validation and state contracts
  romuq-dsp/                Biquads, coefficients, smoothing and EQ processing
  romuq-engine/             Real-time engine, parameter transport and lifecycle
  romuq-analysis/           FFT and non-real-time analysis
  romuq-gui/                GUI model and views
  romuq-plugin/             Host-independent plugin abstractions
  romuq-standalone/         Desktop application
  romuq-cli/                Offline rendering and diagnostics
  romuq-integration-tests/  Cross-layer and integration tests

docs/
  architecture/             Architecture, system schema and P0 gates
  decisions/                Architecture Decision Records
  compatibility/            Host and platform compatibility matrix
  testing/                  Test strategy
  development/              Development workflow

.github/
  workflows/                CI, security and scheduled validation
  ISSUE_TEMPLATE/           Bug and feature templates
```

## Real-time contract

The audio hot path must remain:

- allocation-free;
- non-blocking;
- I/O-free;
- syscall-free;
- panic-free by contract;
- deterministic within documented numerical tolerances.

## Local validation

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-targets
cargo deny check
```

## Naming warning

A preliminary web screening found existing uses of **RomuEq**, including an existing company and an existing brand/design project. Therefore this repository deliberately treats RomuEq as a **provisional codename**, not as a cleared product or company name.

Before public publication, perform a proper trademark, company-name, domain and software-product clearance.
