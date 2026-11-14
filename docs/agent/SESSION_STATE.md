# Session State - RomuEq

**Date:** 2026-11-14  
**Status:** ALL TESTS GREEN (192 tests passing), CI VALIDATED, APP & PLUGINS REBUILT AND INSTALLED

## Architecture & Implementation Overview
1. **Clean-Room Native CoreAudio HAL Driver (`RomuEq.driver`):**
   - Implemented in C11 (`driver/romueq-hal/src/RomuEqDriver.c`, `driver/romueq-hal/Info.plist`).
   - Conforms strictly to Apple `<CoreAudio/AudioServerPlugIn.h>`.
   - Lock-free seqlock for clock timestamps in real-time `GetZeroTimeStamp`.
   - Exact sample frame timestamp match `stamp == sampleTime + 1` in `DoIOOperation` preventing stale audio repeats.
   - CoreAudio configuration changes routed via `host->RequestDeviceConfigurationChange`.
   - COM `QueryInterface` strictly returns standard `HRESULT` (`S_OK`, `E_POINTER`, `E_INVALIDARG`, `E_NOINTERFACE`).
   - Native controls for Master Volume and Mute mapped to macOS keyboard volume keys.
   - **Zero-lag Startup & Silent Presence Check:** `is_romueq_driver_installed_on_disk()` checks driver bundle existence on disk. When present, skips root prompt, installer execution, and `killall coreaudiod`. Fast sample-rate verification (10ms polling, 0ms if already matched) removes launch stalls.
2. **Interactive Equalization Curve (`crates/romueq-gui/src/lib.rs`):**
   - Vertical scale expanded to `[-40.0 dB .. +40.0 dB]` preventing clipping when multiple bands stack.
   - Reference lines at `0 dB` and `±24 dB` band gain boundaries.
   - Screen-space pixel Euclidean distance calculation (`dist_px_sq < 256.0` for 16 pt node drag, `dist_px_sq < 400.0` for 20 pt Q-factor wheel) eliminating accidental adjacent band hijacking.
   - Plot bounds expanded to `[16.0 Hz .. 24 000.0 Hz]` ensuring the `20k` graduation never clips or overlaps neighboring plots. Standard audio frequencies: `20`, `50`, `100`, `200`, `500`, `1k`, `2k`, `5k`, `10k`, `20k`.
3. **Application Lifecycle, Persistence & Audio Engine:**
   - Full DSP state persistence via `~/.romueq/app_state.bin`.
   - Active preset persistence in `~/.romueq/last_preset.txt`.
   - Output device preservation on startup and exit in `~/.romueq/last_output_device.txt`.
   - Dynamic real-time auto-gain calculation via `calculate_auto_gain_compensation_db(&bands, sample_rate)`.
   - Glitch-free audio device refresh in standalone mode without destructive stream interruption.
4. **Professional UI/UX Architecture (`crates/romueq-gui/src/workspace_ui.rs`):**
   - 100% English lingua franca (all bands, filter types, master, and presets).
   - Unified `⏻ BYPASS` LED button eliminating redundancy.
   - Dual plots (EQ Response + Real-Time FFT Spectrum) coupled with a centered vertical stereo VU meter on the right flank (-60 to +6 dBFS with peak hold and clip indicators).
   - Read-only protection for all 15 factory presets (`is_factory_preset`) preventing deletion of default presets.
   - Fixed text edit and dropdown widths preventing UI wrapping and line jitter.
   - **Strictly Symmetric Window Margins:** Exact 8.0px padding on all 4 borders (Left, Right, Top, Bottom) verified by automated headless geometry testing.

