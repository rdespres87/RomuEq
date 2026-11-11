# Handoff - RomuEq

## Current State
- **Standalone (`RomuEq.app`)**: Native macOS standalone equalizer with C11 CoreAudio `AudioServerPlugIn` (`RomuEq.driver`), dual plots (EQ curve `[-40 dB .. +40 dB]` and FFT spectrum), stereo VU-meter, and 15 studio/multimedia factory presets.
- **DAW Plugins (`CLAP`, `VST3`, `AUv2`)**: Implemented in `crates/romueq-plugin` (Rust CLAP + Cocoa `NSView` + Metal `wgpu` + `egui-wgpu` sharing `romueq-gui::workspace_ui` and `~/.romueq/presets/*.req`) and wrapped via `driver/clap-wrapper-romueq` into VST3 and AUv2 (`aufx` / `RmEq` / `Romu`).
- **Verification**:
  - `cargo test --workspace` (411 tests passed: 409 unit/integration + 2 doctests, 0 failed) and `cargo clippy --workspace -- -D warnings` pass cleanly after all fixes.
  - `./scripts/package-macos-plugins.sh` builds, embeds `RomuEq.clap`, inside-out ad-hoc signs, verifies (`codesign --verify --deep --strict`), and installs `RomuEq.clap`, `RomuEq.vst3`, and `RomuEq.component` into `~/Library/Audio/Plug-Ins/{CLAP,VST3,Components}/`.
  - `auval -v aufx RmEq Romu` passes (`AU VALIDATION SUCCEEDED`, 51 automatable parameters, Cocoa view `wrapAsAUV2_inst_cocoaUI_com_romueq_plugin`, stereo `[2, 2]`).
- **Git status**: Changes for the DAW plugin implementation and remediation fixes are ready on disk and awaiting user approval to commit.

## Next Logical Action
1. Confirm with the user whether to create a git commit for the DAW plugin changes.
2. Load **RomuEq** in **Logic Pro** (AUv2), **Ableton Live** (VST3), and **Surge XT / Reaper / Bitwig** (CLAP/VST3) to visually verify the Cocoa Metal UI, mouse drag automation gestures, and preset sync.
