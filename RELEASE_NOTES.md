# Equaform-8 v1.1.0 ("Pro Mixing")

## Highlights

- **Monitor-Only Band Solo** — Isolate any band's frequency range with constant-0 dB-peak RBJ bandpass (Bell) or Butterworth LP/HP (Shelf/Pass) audition filters, parallel multi-band summing, routing-aware audition, and a zero-allocation real-time path. Activated via the amber `S` toggle in each band strip; preserved across preset browsing and cleanly overridden by Bypass.
- **Mid/Side & Per-Band Stereo Routing** — Route each of the 8 bands independently to **Stereo (`ST`)**, **Left (`L`)**, **Right (`R`)**, **Mid (`M`)**, or **Side (`S`)** using zero-unsafe orthonormal L/R ↔ M/S transforms (`FRAC_1_SQRT_2`), a conditional M/S pass with zero overhead in pure Stereo, 8 new automatable DAW parameters (**59 total**), and a color-coded routing button beside Slope.
- **Instant A/B Compare** — Click-free `A | B` slot toggle and dynamic `A→B` / `B→A` copy button in the top toolbar (shared across Standalone and DAW plugins), emitting diff-only parameter updates while keeping monitor-only Solo independent.
- **Vectorized Stereo DSP & Safety Hardening** — Zero-unsafe auto-vectorized stereo biquad kernels (`-49.5%` to `-54.2%` latency, `~2.0x–2.2x` stereo throughput on ARM64), sorted preset menu (user presets first, factory presets alphabetically), compile-time typestate `Engine<Unconfigured | Prepared | Running>`, and exhaustive FFI/`unsafe` safety documentation (`0` warnings under `-D clippy::undocumented_unsafe_blocks`, 481 passing workspace tests).

## Installation

| File | Description |
|------|-------------|
| `Equaform-8-v1.1.0.dmg` | macOS installer (AUv2, VST3, CLAP plugins + `Equaform-8.app` standalone + 15 factory presets, Apple Silicon) |
| `Equaform-8-v1.1.0.dmg.sha256` | SHA-256 checksum for the macOS DMG |
| `Equaform-8-Plugins-Windows-v1.1.0.exe` | Windows x64 installer (VST3, CLAP plugins + 15 factory presets) |
| `Equaform-8-Plugins-Windows-v1.1.0.exe.sha256` | SHA-256 checksum for the Windows installer |

**Requirements:** macOS 12.0+ (Apple Silicon) or Windows 10/11 (x64).

```bash
# Verify macOS download integrity
shasum -a 256 -c Equaform-8-v1.1.0.dmg.sha256
```

## Notes

`v1.1.0` is 100% backward-compatible with `v1.0.0` presets and DAW project states. Legacy presets and sessions without per-band routing metadata default automatically to `Stereo`, and existing parameter IDs remain unchanged.
