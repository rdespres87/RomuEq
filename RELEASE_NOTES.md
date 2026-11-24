# Equaform-8 v1.0.0

## Highlights

- **8-Band Parametric EQ** — Bell, Low/High Shelf, Low/High Pass, Notch, and Band Pass filters built on 64-bit floating-point TDF-II biquad DSP with a zero-allocation real-time audio engine (ADR-007), lock-free triple-buffer parameter sync, click-free 5 ms equal-power bypass crossfade and mute ramp, and RMS auto-gain compensation.
- **Real-Time Spectrum Analyzer** — FFT-based display with peak hold, tilt/slope adjustment, and pre/post-EQ monitoring, rendered via GPU-accelerated wgpu/egui UI with adaptive FPS throttling (60 FPS interactive, 15 FPS idle, 0 FPS minimized).
- **Native DAW Plugin Formats & Standalone App** — AUv2, VST3, and CLAP plugins plus `Equaform-8.app` standalone on macOS (Apple Silicon); VST3 and CLAP plugins on Windows (x64).
- **15 Factory Presets & Multilingual UI** — Atomic CRC32-framed state persistence, JSON preset import/export, and native 4-language interface (English, French, German, Spanish).
- **Initial Public Release** — First stable release of the Equaform-8 audio processing suite.

## Installation

| File | Description |
|------|-------------|
| `Equaform-8-v1.0.0.dmg` | macOS installer (AUv2, VST3, CLAP plugins + `Equaform-8.app` standalone + 15 factory presets, Apple Silicon) |
| `Equaform-8-v1.0.0.dmg.sha256` | SHA-256 checksum for the macOS DMG |
| `Equaform-8-Plugins-Windows-v1.0.0.exe` | Windows x64 installer (VST3, CLAP plugins + 15 factory presets) |
| `Equaform-8-Plugins-Windows-v1.0.0.exe.sha256` | SHA-256 checksum for the Windows installer |

**Requirements:** macOS 12.0+ (Apple Silicon) or Windows 10/11 (x64).

```bash
# Verify macOS download integrity
shasum -a 256 -c Equaform-8-v1.0.0.dmg.sha256
```

## Notes

This is the initial public release of Equaform-8 (`v1.0.0`). No migration steps are required.
