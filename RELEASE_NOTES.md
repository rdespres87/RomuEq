# Equaform-8 v1.2.0 ("Musical Control & Workflow")

## New Features

- **Dual-Mode Auto-Gain (Perceptual & Musical Anti-Clip)** — Choose between **Perceptual** (maintains perceived loudness across Bypass) and **Anti-Clip** (musical headroom protection weighted by a frequency-aware envelope, capped at <= 0 dB to prevent clipping on hot boosts while keeping sub-bass and air musical). Exposed across Standalone, GUI, and DAW automation (**60** automatable parameters, `ParamId(7)`).
- **Bounded Undo/Redo History (`Cmd/Ctrl+Z`, `Cmd/Ctrl+Shift+Z` / `Ctrl+Y`)** — 64-step gesture-coalescing undo/redo controller shared by Standalone and DAW plugins with compact toolbar controls (`<-` / `->`). Continuous slider/curve drags coalesce into a single undo step, text fields keep native text undo, and monitoring/bypass toggles (`Bypass`, `Mute`, `Solo`) are cleanly excluded from EQ history.

## Improvements & Hardening

- **Zero Missing UI Glyphs** — Replaced unsupported Unicode toolbar symbols (`⏻`, `→`, `↶`, `↷`) with verified `egui`-native characters (`•`, `A>B`/`B>A`, `<-`/`->`) across all 4 languages (`EN`, `FR`, `DE`, `ES`).
- **Architecture & Modularity (Maturity Audit 9.4/10)** — Split `equaform-plugin/src/lib.rs` (`processor.rs`) and `equaform-gui/src/standalone.rs` (`standalone/mod.rs` + `engine_client.rs`) so 100% of production source files stay under 1,000 LOC; replaced 8-argument `WgpuSurfaceHost::new` with `WgpuSurfaceConfig` (**0** `#[allow(clippy::...)]` workspace-wide).
- **Atomic Driver Installer Staging** — macOS HAL driver installation now writes its temporary `.pkg` atomically to a PID-scoped path with hardened shell/AppleScript escaping and `coreaudiod` readiness polling.

## Installation

| File | Description |
|------|-------------|
| `Equaform-8-v1.2.0.dmg` | macOS installer (AUv2, VST3, CLAP plugins + `Equaform-8.app` standalone + 15 factory presets, Apple Silicon) |
| `Equaform-8-v1.2.0.dmg.sha256` | SHA-256 checksum for the macOS DMG |
| `Equaform-8-Plugins-Windows-v1.2.0.exe` | Windows x64 installer (VST3, CLAP plugins + 15 factory presets) |
| `Equaform-8-Plugins-Windows-v1.2.0.exe.sha256` | SHA-256 checksum for the Windows installer |

**Requirements:** macOS 12.0+ (Apple Silicon) or Windows 10/11 (x64).

```bash
# Verify macOS download integrity
shasum -a 256 -c Equaform-8-v1.2.0.dmg.sha256
```

## Notes

`v1.2.0` is 100% backward-compatible with `v1.0.0`, `v1.1.0`, and `v1.1.1` presets and DAW project states (`auto_gain_mode` defaults to `Perceptual` when absent from legacy states).
