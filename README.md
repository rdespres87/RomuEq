# RomuEq — PROVISIONAL PROJECT NAME

> **Naming status: NOT CLEARED.** RomuEq is currently used as a working name only. Public release, trademark filing, domain acquisition, and repository publication must wait for final name clearance.

Real-time 8-band parametric equalizer written in Rust, available as a **native macOS standalone application** (with CoreAudio HAL virtual driver) and as **DAW plugins (`CLAP`, `VST3`, `AUv2`)**, plus an offline CLI processor.

## Implemented Targets & Features

- **DAW Plugins (`CLAP`, `VST3`, `AUv2`)**
  - Native **CLAP** implementation in `crates/romueq-plugin` with an embedded macOS Cocoa (`NSView`) + Metal (`wgpu` / `egui-wgpu`) editor (`1180×560`).
  - **VST3** (Ableton Live, Cubase, Reaper, Bitwig) and **Audio Unit v2** (`aufx` / `RmEq` / `Romu` for Logic Pro, GarageBand, MainStage) generated via `clap-wrapper` with an embedded `.clap` bundle.
  - **51 automatable parameters** (8 bands × Active/Type/Frequency/Gain/Q/Slope + Global Gain, Output Trim, Bypass) with sample-accurate sub-block automation, Touch/Latch UI gestures, text value parsing, and versioned JSON state persistence.
- **Standalone macOS Application (`RomuEq.app`)**
  - System-wide EQ powered by a native C11 CoreAudio `AudioServerPlugIn` virtual audio driver (`driver/romueq-hal`) and drift-compensated CoreAudio playback.
- **CLI / Offline Renderer (`romueq-cli`)**
  - Offline WAV processing and diagnostics using the shared DSP engine.
- **DSP, Analysis & Shared UI (`romueq-gui`)**
  - 8-band minimum-phase EQ (`Bell`, `LowShelf`, `HiShelf`, `HiPass`, `LowPass`; 6/12/18/24 dB/oct slopes), `[-40 dB .. +40 dB]` response curve plot, real-time FFT spectrum analyzer, and stereo L/R VU-meter.
  - Lock-free audio hot path (`ADR-007`: zero allocation, non-blocking ring buffers & atomic snapshots).
  - **15 factory presets** and user presets synchronized across standalone and plugins in `~/.romueq/presets/*.req`.

## Future Scope

- Dynamic EQ, linear-phase mode, oversampling, and Mid/Side processing
- Windows & Linux packaging for `CLAP` / `VST3` plugins
- Audio Unit v3 (`AUv3`) and multichannel support

## Repository Layout

```text
crates/
  romueq-common/             Shared types, parameter IDs, lock-free rings, state schema
  romueq-dsp/                Biquads, coefficients, smoothing, and EQ processing
  romueq-engine/             Real-time engine lifecycle and parameter transport
  romueq-analysis/           Lock-free FFT spectrum and VU metering
  romueq-gui/                Shared egui workspace UI (`workspace_ui`) and preset I/O
  romueq-plugin/             CLAP plugin implementation + Cocoa/Metal plugin editor
  romueq-standalone/         macOS standalone desktop application
  romueq-cli/                Offline WAV renderer and diagnostics
  romueq-integration-tests/  Cross-layer and integration tests

driver/
  romueq-hal/                Native C11 CoreAudio AudioServerPlugIn virtual driver
  clap-wrapper/              Vendored clap-wrapper 0.17.0 (VST3 & AUv2 wrapper)
  clap-wrapper-romueq/       CMake build configuration for RomuEq VST3 & AUv2 bundles

scripts/
  build-driver.sh            Builds universal binary CoreAudio HAL driver
  package-macos-app.sh       Packages RomuEq.app (standalone)
  package-macos-plugins.sh   Builds, inside-out signs, and installs CLAP, VST3 & AUv2

docs/
  architecture/              Architecture, system schema, and design specs
  decisions/                 Architecture Decision Records (ADRs)
  agent/                     Project handoff notes (`HANDOFF.md`)
```

## Building & Installing on macOS

### DAW Plugins (`CLAP`, `VST3`, `AUv2`)

Requires Rust toolchain, CMake (`>= 3.22`), Ninja, and Xcode Command Line Tools:

```bash
./scripts/package-macos-plugins.sh
```

This builds `libromueq_plugin.dylib`, wraps it into `RomuEq.clap`, `RomuEq.vst3`, and `RomuEq.component`, performs inside-out ad-hoc codesigning, refreshes `AudioComponentRegistrar`, and installs them to:

- `~/Library/Audio/Plug-Ins/CLAP/RomuEq.clap`
- `~/Library/Audio/Plug-Ins/VST3/RomuEq.vst3`
- `~/Library/Audio/Plug-Ins/Components/RomuEq.component`

Verify the Audio Unit registration with Apple's validator:

```bash
auval -v aufx RmEq Romu
```

### Standalone Application (`RomuEq.app`)

```bash
./scripts/package-macos-app.sh
```

## Real-Time Contract

The audio hot path must remain:

- allocation-free;
- non-blocking;
- I/O-free;
- syscall-free;
- panic-free by contract;
- deterministic within documented numerical tolerances.

## Local Validation

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
auval -v aufx RmEq Romu
```

## Naming Warning

A preliminary web screening found existing uses of **RomuEq**, including an existing company and an existing brand/design project. Therefore this repository deliberately treats RomuEq as a **provisional codename**, not as a cleared product or company name.

Before public publication, perform a proper trademark, company-name, domain and software-product clearance.
