# Handoff - Equaform

## Current State
- **DAW Plugins (`CLAP`, `VST3`, `AUv2`)**: Implemented in `crates/equaform-plugin` (Rust CLAP + Cocoa `NSView` + Metal `wgpu` + `egui-wgpu` sharing `equaform-gui::workspace_ui`) and wrapped via `driver/clap-wrapper-equaform` into VST3 and AUv2 (`aufx` / `Eqf8` / `Eqfm`).
- **Commercial Identity — Equaform-8 by Equaform Audio**:
  - Official commercial name: `Equaform-8` (Tagline: "The Geometry of Pure Balance").
  - Vendor identity: `Equaform Audio` (`com.equaform.audio.equalizer`).
  - Full backward compatibility for project state and preset files (transparently supports both `Equaform-8` and legacy `Equaform`).
  - Bundles packaged and installed: `Equaform-8.clap`, `Equaform-8.vst3`, `Equaform-8.component`, `Equaform-8.app`.
- **Commercial UI/UX & Standalone Overhaul**:
  - 100% English lingua franca across all controls, filter types, bands, and tooltips.
  - Streamlined `⏻ BYPASS` control with status LED.
  - Vertical stereo VU-meter on right flank (-60 to +6 dBFS with peak hold and clip indicator).
  - Frequency axis margined to `[16 Hz .. 24 kHz]` with standard audio ticks (`20`, `50`, `100`, `200`, `500`, `1k`, `2k`, `5k`, `10k`, `20k`) completely visible without clipping.
  - Read-only protection on factory presets (cannot be deleted) with stable layout widths.
  - Glitch-free audio device refresh in standalone mode.
  - Strictly symmetric 8.0px margins on all 4 borders (Left, Right, Top, Bottom) verified by unit tests.
  - Instant zero-lag standalone startup: skips spurious administrator prompt / `killall coreaudiod` when driver is already present on disk; fast 10ms sample-rate lock without stalling UI.
  - Zero-stutter audio streaming: CoreAudio HAL loopback SafetyOffset set to 0 (v1.1.0 driver), anti-chopping full-block delivery, and direct 1:1 bit-exact streaming fast-path.
  - Spectrum drawing bug and Auto-Gain compensation calculation fixed and verified.
- **Verification**:
  - `cargo test --workspace` (192 tests passing: all unit/integration tests + doctests) and `cargo clippy --workspace --all-targets -- -D warnings` pass 100% clean with zero warnings.
  - `cargo fmt --check` passes cleanly.
  - `./scripts/package-macos-plugins.sh` successfully rebuilt and installed updated CLAP, VST3, and AUv2 bundles (`Equaform-8.clap`, `Equaform-8.vst3`, `Equaform-8.component`) into `~/Library/Audio/Plug-Ins/`.
  - `./scripts/package-macos-app.sh` successfully packaged and codesigned `Equaform-8.app`.
- **Git status**: Ready for review and commit validation.

## Next Logical Action
1. Test the new UI in Ableton Live to confirm visual layout, vertical VU-meter, and 20k margins.
2. Ask user authorization to commit the changes via Security Gate 3.
