# Session State - RomuEq

**Date:** 2026-11-14  
**Status:** ALL TESTS GREEN (192 tests passing), CI VALIDATED, ZERO-STUTTER DRIVER V1.1.0 PACKAGED & INSTALLED

## Architecture & Implementation Overview
1. **Clean-Room Native CoreAudio HAL Driver v1.1.0 (`RomuEq.driver`):**
   - Implemented in C11 (`driver/romueq-hal/src/RomuEqDriver.c`, `driver/romueq-hal/Info.plist`).
   - Conforms strictly to Apple `<CoreAudio/AudioServerPlugIn.h>`.
   - Lock-free seqlock for clock timestamps in real-time `GetZeroTimeStamp`.
   - **Zero-Stutter Synchronous Loopback:** Configured `kAudioDevicePropertySafetyOffset` to 0 for both Input and Output scopes (BlackHole architectural pattern). Input and Output share the exact same `sampleTime`, eliminating the artificial 512-frame time offset and startup audio stutter.
   - Exact frame consumption: `ReadInput` reads `sampleTime`, copies to destination buffer, and immediately zeros the ring buffer slot, ensuring perfect silence when playback pauses without stale looping.
   - CoreAudio configuration changes routed via `host->RequestDeviceConfigurationChange`.
   - COM `QueryInterface` strictly returns standard `HRESULT` (`S_OK`, `E_POINTER`, `E_INVALIDARG`, `E_NOINTERFACE`).
   - Native controls for Master Volume and Mute mapped to macOS keyboard volume keys.
   - **Zero-lag Startup & Silent Presence Check:** `is_romueq_driver_installed_on_disk()` checks driver bundle existence and version (v1.1.0 / build 2). When present, skips root prompt, installer execution, and `killall coreaudiod`. Fast sample-rate verification (10ms polling, 0ms if already matched) removes launch stalls.
2. **Real-Time Streaming Engine (`crates/romueq-gui/src/audio.rs`):**
   - **Anti-Chopping Full-Block Protection:** In `ContinuousPiDriftController::read_from`, partial blocks (`available < count`) are no longer chopped with exponential volume decay. Clean silence is delivered for the cycle, allowing the buffer to hold a full block for the next cycle without stutter.
   - **Bit-Exact Direct Fast-Path:** When input and output sample rates match (e.g. 44.1 kHz / 48 kHz) within a generous ±512 frames deadband, frames transfer directly via `consumer.pop_slice` without fractional interpolation jitter.
   - **Headroom Tuning:** Buffer target optimized to 2048 frames (~46 ms) for rapid initial stabilization.
3. **Interactive Equalization Curve (`crates/romueq-gui/src/lib.rs`):**
   - Vertical scale expanded to `[-40.0 dB .. +40.0 dB]` preventing clipping when multiple bands stack.
   - Reference lines at `0 dB` and `±24 dB` band gain boundaries.
   - Screen-space pixel Euclidean distance calculation (`dist_px_sq < 256.0` for 16 pt node drag, `dist_px_sq < 400.0` for 20 pt Q-factor wheel) eliminating accidental adjacent band hijacking.
   - Plot bounds expanded to `[16.0 Hz .. 24 000.0 Hz]` ensuring the `20k` graduation never clips or overlaps neighboring plots. Standard audio frequencies: `20`, `50`, `100`, `200`, `500`, `1k`, `2k`, `5k`, `10k`, `20k`.
4. **Application Lifecycle, Persistence & Audio Engine:**
   - Full DSP state persistence via `~/.romueq/app_state.bin`.
   - Active preset persistence in `~/.romueq/last_preset.txt`.
   - Output device preservation on startup and exit in `~/.romueq/last_output_device.txt`.
   - Dynamic real-time auto-gain calculation via `calculate_auto_gain_compensation_db(&bands, sample_rate)`.
   - Glitch-free audio device refresh in standalone mode without destructive stream interruption.
5. **Professional UI/UX Architecture (`crates/romueq-gui/src/workspace_ui.rs`):**
   - 100% English lingua franca (all bands, filter types, master, and presets).
   - Unified `⏻ BYPASS` LED button eliminating redundancy.
   - Dual plots (EQ Response + Real-Time FFT Spectrum) coupled with a centered vertical stereo VU meter on the right flank (-60 to +6 dBFS with peak hold and clip indicators).
   - Read-only protection for all 15 factory presets (`is_factory_preset`) preventing deletion of default presets.
   - Fixed text edit and dropdown widths preventing UI wrapping and line jitter.
   - **Strictly Symmetric Window Margins:** Exact 8.0px padding on all 4 borders (Left, Right, Top, Bottom) verified by automated headless geometry testing.

