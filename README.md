# Equaform-8

![Equaform-8 Parametric Equalizer](docs/assets/equaform-8-gui.jpg)

Real-time 8-band parametric equalizer written in Rust, available as **DAW plugins (`CLAP`, `VST3`, `AUv2`)** on macOS and **DAW plugins (`CLAP`, `VST3`)** on Windows x64, plus a **native macOS standalone application** (`Equaform-8.app` with CoreAudio HAL virtual driver) and an offline CLI processor.

## Platform Support Matrix

| Target | macOS (Apple Silicon / Intel) | Windows (x64) |
| :--- | :---: | :---: |
| **CLAP Plugin** | Yes (`Equaform-8.clap`) | Yes (`Equaform-8.clap`) |
| **VST3 Plugin** | Yes (`Equaform-8.vst3`) | Yes (`Equaform-8.vst3`) |
| **AUv2 Plugin** | Yes (`Equaform-8.component`) | N/A |
| **Standalone App** | Yes (`Equaform-8.app` + `Equaform.driver`) | **No (Plugins only)** |
| **Release Deliverable** | `Equaform-8-vX.Y.Z.dmg` (Plugins + Standalone) | `Equaform-8-Plugins-Windows-vX.Y.Z.exe` (Plugins only) |

> **Windows Scope Note:** Windows releases intentionally ship **DAW plugins only (`VST3` and `CLAP`)** with the 15 factory presets. The standalone system-wide application is **macOS-only** because it relies on the native CoreAudio HAL virtual driver (`driver/equaform-hal`).

## Implemented Targets & Features

- **DAW Plugins (`CLAP`, `VST3`, `AUv2`)**
  - Native **CLAP** implementation in `crates/equaform-plugin` with embedded **macOS Cocoa (`NSView`) + Metal** and **Windows Win32 (`HWND`) + DirectX 12 / Vulkan** editors (`wgpu` / `egui-wgpu`, `1180×560`).
  - **VST3** (Ableton Live, Cubase, Reaper, Bitwig, FL Studio) on macOS & Windows, and **Audio Unit v2** (`aufx` / `Eqf8` / `Eqfm` for Logic Pro, GarageBand, MainStage) on macOS, generated via `clap-wrapper`.
  - **51 automatable parameters** (8 bands × Active/Type/Frequency/Gain/Q/Slope + Global Gain, Output Trim, Bypass) with sample-accurate sub-block automation, Touch/Latch UI gestures, text value parsing, and versioned CRC32 binary + JSON state persistence.
- **Standalone macOS Application (`Equaform-8.app` — macOS only)**
  - System-wide EQ powered by a native C11 CoreAudio `AudioServerPlugIn` virtual audio driver (`driver/equaform-hal`) and drift-compensated CoreAudio playback.
- **CLI / Offline Renderer (`equaform-cli`)**
  - Offline WAV processing and diagnostics using the shared DSP engine.
- **DSP, Analysis & Shared UI (`equaform-gui`)**
  - 8-band minimum-phase EQ (`Bell`, `LowShelf`, `HiShelf`, `HiPass`, `LowPass`; 6/12/18/24 dB/oct slopes), `[-40 dB .. +40 dB]` response curve plot (dirty-flag cached), real-time FFT spectrum analyzer, stereo L/R VU-meter, and native zero-allocation i18n (`EN` / `FR` / `DE`).
  - Lock-free audio hot path (`ADR-007`: zero allocation, non-blocking ring buffers & atomic snapshots).
  - **15 factory presets** and user presets synchronized across standalone and plugins (`~/.equaform/presets/*.req` on macOS, `%APPDATA%\Equaform\presets\*.req` on Windows).

## Future Scope

- Dynamic EQ, linear-phase mode, oversampling, and Mid/Side processing
- Linux packaging for `CLAP` / `VST3` plugins
- Audio Unit v3 (`AUv3`) and multichannel support

## Repository Layout

```text
crates/
  equaform-common/             Shared types, parameter IDs, lock-free rings, state schema
  equaform-dsp/                Biquads, coefficients, smoothing, and EQ processing
  equaform-engine/             Real-time engine lifecycle and parameter transport
  equaform-analysis/           Lock-free FFT spectrum and VU metering
  equaform-gui/                Shared egui workspace UI (`workspace_ui`) and preset I/O
  equaform-plugin/             CLAP plugin implementation + Cocoa/Metal plugin editor
  equaform-standalone/         macOS standalone desktop application
  equaform-cli/                Offline WAV renderer and diagnostics
  equaform-integration-tests/  Cross-layer and integration tests

driver/
  equaform-hal/                Native C11 CoreAudio AudioServerPlugIn virtual driver
  clap-wrapper/                Vendored clap-wrapper 0.17.0 (VST3 & AUv2 wrapper)
  clap-wrapper-equaform/       CMake build configuration for Equaform VST3 & AUv2 bundles

scripts/
  build-driver.sh              Builds universal binary CoreAudio HAL driver
  package-macos-app.sh         Packages Equaform-8.app (standalone)
  package-macos-plugins.sh     Builds, inside-out signs, and installs CLAP, VST3 & AUv2

docs/
  architecture/                Architecture, system schema, and design specs
  decisions/                   Architecture Decision Records (ADRs)
  branding/                    Brand identity and positioning specs
  agent/                       Project handoff notes (`HANDOFF.md`)
```

## Building & Installing on macOS

### DAW Plugins (`CLAP`, `VST3`, `AUv2`)

Requires Rust toolchain, CMake (`>= 3.22`), Ninja, and Xcode Command Line Tools:

```bash
./scripts/package-macos-plugins.sh
```

This builds `libequaform_plugin.dylib`, wraps it into `Equaform-8.clap`, `Equaform-8.vst3`, and `Equaform-8.component`, performs inside-out ad-hoc codesigning, refreshes `AudioComponentRegistrar`, and installs them to:

- `~/Library/Audio/Plug-Ins/CLAP/Equaform-8.clap`
- `~/Library/Audio/Plug-Ins/VST3/Equaform-8.vst3`
- `~/Library/Audio/Plug-Ins/Components/Equaform-8.component`

Verify the Audio Unit registration with Apple's validator:

```bash
auval -v aufx Eqf8 Eqfm
```

### Standalone Application (`Equaform-8.app` — macOS only)

```bash
./scripts/package-macos-app.sh
```

### Combined Distribution DMG (Plugins + Standalone)

```bash
./scripts/package-distribution-plugins.sh 1.0.0
```

This generates `target/release/dist/Equaform-8-v1.0.0.dmg` (and its `.sha256` checksum) containing both the DAW plugins installer (`Equaform-8-Plugins-Installer.pkg` for `AUv2`, `VST3`, `CLAP`, and presets) and the standalone `Equaform-8.app` bundle.

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
auval -v aufx Eqf8 Eqfm
```

## License & Trademarks

- Distributed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
- VST is a registered trademark of Steinberg Media Technologies GmbH.

