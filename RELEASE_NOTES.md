# Equaform-8 v1.1.1 ("Reliability & Polish")

## Fixes & Improvements

- **Preset Persistence Fix** — Monitor-only Band Solo IDs (`106..=806`) are now filtered out on save and stripped on load/import, ensuring newly saved `v1.1.0+` user presets reload reliably across app restarts without validation rejection.
- **Accurate Stereo VU-Metering** — Added stride-aware planar analysis (`AnalysisFrame::compute_planar`) in `equaform-analysis` and wired it into `EngineLifecycle::process`, eliminating L/R metering distortion when `block_size < max_block_size`.
- **Click-Free Band Solo Crossfade** — Replaced hard solo toggles with an equal-power sin/cos crossfade (`SoloTransitionState`) using a pre-allocated buffer (zero audio-thread allocations) and smooth auto-gain compensation scaling.
- **Bounded File Reads (16 MiB)** — Added TOCTOU-safe `read_to_string_bounded` across preset, state, and language loaders to prevent unbounded memory allocation on oversized files.
- **Complete 4-Language UI Localization** — Localized all Band Solo, A/B Compare, and Mid/Side channel routing tooltips across English, French, German, and Spanish.
- **DSP Loop Optimizations** — Hoisted `update_auto_gain_db()` out of the per-band automation loop and hoisted Mid/Side transforms out of the per-band stereo solo loop (single `ms_to_lr_in_place` pass per block).

## Installation

| File | Description |
|------|-------------|
| `Equaform-8-v1.1.1.dmg` | macOS installer (AUv2, VST3, CLAP plugins + `Equaform-8.app` standalone + 15 factory presets, Apple Silicon) |
| `Equaform-8-v1.1.1.dmg.sha256` | SHA-256 checksum for the macOS DMG |
| `Equaform-8-Plugins-Windows-v1.1.1.exe` | Windows x64 installer (VST3, CLAP plugins + 15 factory presets) |
| `Equaform-8-Plugins-Windows-v1.1.1.exe.sha256` | SHA-256 checksum for the Windows installer |

**Requirements:** macOS 12.0+ (Apple Silicon) or Windows 10/11 (x64).

```bash
# Verify macOS download integrity
shasum -a 256 -c Equaform-8-v1.1.1.dmg.sha256
```

## Notes

`v1.1.1` is 100% backward-compatible with `v1.0.0` and `v1.1.0` presets and DAW project states. Any user `.req` preset saved under `v1.1.0` that contained ephemeral Band Solo IDs will now load automatically without manual editing.
