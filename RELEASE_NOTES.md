# Equaform-8 v1.3.0 ("Linear-Phase FIR EQ")

## New Features

- **3-Mode Phase Processing (`Zero Latency`, `Linear Low`, `Linear High`)** — Switch seamlessly between zero-latency minimum-phase IIR (`0 samples`), low-latency linear-phase FIR (`2048 taps / 1024 samples`), and high-precision linear-phase FIR (`4096 taps / 2048 samples`) for transparent mastering and sub-bass phase coherence.
- **Zero-Allocation UPOLS Stereo & Mid/Side FIR Convolver** — `#![forbid(unsafe_code)]` 2x2 partitioned overlap-save convolution (`B=256`, `FFT=512`) with packed stereo FFT, asynchronous lock-free FIR kernel synthesis, and dual-bank crossfading.
- **DAW Plugin Delay Compensation (PDC) & Automation** — Exposes `Processing Mode` (`ParamId(4)`) across **61** automatable parameters and reports exact sample latency via `CLAP_EXT_LATENCY` (bridged to VST3 and AUv2), paired with a latency-aligned dry bypass (`DryDelayLine`).
- **Master Bar Mode Selector & Live Latency Tooltips** — Compact 3-mode selector in the master bar displaying real-time latency in samples and milliseconds across `EN`, `FR`, `DE`, and `ES`.

## Improvements & Hardening

- **Click-Free Mode, Routing & Solo Transitions** — Smooth 5 ms biquad coefficient and gain ramps across band toggles, `Stereo / Left / Right / Mid / Side` routing switches (normal and Band Solo), `MinimumPhase -> LinearPhase` pipeline warmup without silence dips, and continuous linear-phase feeding during Band Solo.
- **Standalone Launch/Exit & Underrun Polish** — 150 ms cosine S-curve fade-in on first non-silent audio and 150 ms fade-out on graceful shutdown (`Ctrl+C`, window close, or Quit), plus smooth exponential decay on ring-buffer underruns.
- **Zero-Allocation UI Snapshot Reuse** — Reuses `SharedEditorState::cached_snapshot` per frame without heap allocations.
- **Maturity & Verification (`9.6 / 10`)** — **565** workspace tests passing, **91.72%** core line coverage, `0` clippy warnings, `cargo deny` clean, and Apple `auval` (`AU VALIDATION SUCCEEDED`).

## Installation

| File | Description |
|------|-------------|
| `Equaform-8-v1.3.0.dmg` | macOS installer (AUv2, VST3, CLAP plugins + `Equaform-8.app` standalone + 15 factory presets, Apple Silicon) |
| `Equaform-8-v1.3.0.dmg.sha256` | SHA-256 checksum for the macOS DMG |
| `Equaform-8-Plugins-Windows-v1.3.0.exe` | Windows x64 installer (VST3, CLAP plugins + 15 factory presets) |
| `Equaform-8-Plugins-Windows-v1.3.0.exe.sha256` | SHA-256 checksum for the Windows installer |

**Requirements:** macOS 12.0+ (Apple Silicon) or Windows 10/11 (x64).

```bash
# Verify macOS download integrity
shasum -a 256 -c Equaform-8-v1.3.0.dmg.sha256
```

## Notes

`v1.3.0` is 100% backward-compatible with `v1.0.0`, `v1.1.0`, `v1.1.1`, and `v1.2.0` presets and DAW project states (`processing_mode` defaults to `ZeroLatency` when absent from legacy states).
