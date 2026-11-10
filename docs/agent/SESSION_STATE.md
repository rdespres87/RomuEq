# Session State - RomuEq

**Date:** 2026-11-10
**Status:** ALL TESTS GREEN, GRILL APPROVED, COMMIT AUTHORIZED & EXECUTED (`5d9061d`)

## Architecture & Implementation Overview
1. **Clean-Room Native CoreAudio HAL Driver (`RomuEq.driver`):**
   - Implemented in C11 (`driver/romueq-hal/src/RomuEqDriver.c`, `driver/romueq-hal/Info.plist`).
   - Conforms strictly to Apple `<CoreAudio/AudioServerPlugIn.h>`.
   - Lock-free seqlock for clock timestamps in real-time `GetZeroTimeStamp`.
   - Exact sample frame timestamp match `stamp == sampleTime + 1` in `DoIOOperation` preventing stale audio repeats.
   - CoreAudio configuration changes routed via `host->RequestDeviceConfigurationChange`.
   - COM `QueryInterface` strictly returns standard `HRESULT` (`S_OK`, `E_POINTER`, `E_INVALIDARG`, `E_NOINTERFACE`).
   - Native controls for Master Volume and Mute mapped to macOS keyboard volume keys.
2. **Interactive Equalization Curve (`crates/romueq-gui/src/lib.rs`):**
   - Vertical scale expanded to `[-40.0 dB .. +40.0 dB]` preventing clipping when multiple bands stack.
   - Reference lines at `0 dB` and `±24 dB` band gain boundaries.
   - Screen-space pixel Euclidean distance calculation (`dist_px_sq < 256.0` for 16 pt node drag, `dist_px_sq < 400.0` for 20 pt Q-factor wheel) eliminating accidental adjacent band hijacking.
3. **Application Lifecycle & Persistence:**
   - Full DSP state persistence via `~/.romueq/app_state.bin`.
   - Active preset persistence in `~/.romueq/last_preset.txt`.
   - Output device preservation on startup and exit in `~/.romueq/last_output_device.txt`.
   - Graceful coreaudiod daemon reload via `killall coreaudiod`.

