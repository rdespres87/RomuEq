# Tasks & Progress - RomuEq

- [x] Align 8-band EQ nodes with response curve
- [x] Compact window layout (`1180 × 560`) with full vertical band faders visible
- [x] Side-by-side plots for Equalization and FFT Spectrum
- [x] Stereo L/R level meters replacing text peak readout
- [x] Remove obsolete demo signal selector
- [x] Real-time delivery of EQ curve updates without dropped frames (`ensure_parameters_synced`)
- [x] Expand EQ curve vertical axis to `[-40 dB .. +40 dB]`
- [x] Implement clean-room native macOS CoreAudio HAL virtual loopback driver (`RomuEq.driver`)
- [x] Pass adversarial Inquisitor Grill session with zero critical or major defects
- [x] Validate all 188 workspace unit and integration tests, format, and clippy with zero warnings
- [x] Commit changes to git repository (`2a90ca7`)
- [x] Add 10 studio & multimedia factory presets (EDM, Rock, Hip-Hop, Jazz, Pop, Podcast, Cinéma, Gaming, Harman, Night Mode)
- [x] Commercial feasibility study (licensing, IP, code protection, distribution, packaging)
- [x] Fix VST3 spectrum analysis drawing (FFT full bins, DFT gain normalization, idle decay)
- [x] Fix Auto-Gain real-time analytical compensation injection in VST3/AU plugin UI
- [x] Complete UI/UX professional refactoring:
  - [x] 100% English lingua franca (bands, filter types, master, tooltips)
  - [x] Streamlined Bypass control (LED power button) and Mute in standalone
  - [x] Remove redundant subtitle banner, maximizing plot vertical space
  - [x] Vertical stereo VU-meter on right side with dBFS scale (-60 to +6 dBFS)
  - [x] Fix 20 kHz plot margin truncation / overlap
  - [x] Read-only protected factory presets (cannot be deleted) with stable layout widths
  - [x] Glitch-free audio device refresh without stream interruption
  - [x] Perfectly symmetric 8.0px margins on all 4 borders (Left, Right, Top, Bottom)
  - [x] Instant zero-lag standalone startup (no spurious root prompts or coreaudiod kills when driver is installed; optimized sample-rate lock)
  - [x] Fix audio stutter / micro-chopping at startup: zero-offset CoreAudio HAL loopback (driver v1.1.0), anti-chopping full-block delivery, and direct 1:1 bit-exact streaming fast-path
- [x] Rebuild and install all macOS plugin bundles (`RomuEq.vst3`, `RomuEq.component`, `RomuEq.clap`) and standalone application (`RomuEq.app`)

