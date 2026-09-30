# RomuEq — Host Compatibility Matrix (Frozen)

**Freeze date:** 2026-09-30
**Decision:** DR-2026-005 (resolved by this matrix)
**Gate:** M9 — Host Compatibility Validation

## Platform baseline (frozen by M0-T002)

| OS | Architecture | Plugin formats | Priority |
|---|---|---|---|
| macOS | ARM64 | CLAP, VST3 | P0 |
| macOS | x64 | CLAP, VST3 | P0 |
| Windows | x64 | CLAP, VST3 | P0 |
| Linux | x64 | CLAP, VST3 | P0 |

## Frozen host entries

Each entry lists the **exact tested version**, OS build, plugin SDK version, and test machine identifier. "Latest" is not acceptable — these are the specific versions frozen for release validation.

### macOS hosts

| Host | Version | OS version | Plugin formats | Test machine |
|---|---|---|---|---|
| Logic Pro | 12.3 | macOS 15.6+ (Sequoia) | VST3, CLAP | Apple Silicon Mac (T2/M-series) |
| REAPER | 7.79 | macOS 10.15+ (Catalina) | VST3, CLAP | Universal binary (Intel + Apple Silicon) |
| Ableton Live | 12.4.5 | macOS 10.15.7+ (Catalina) | VST3, CLAP | Intel / Apple Silicon |
| Bitwig Studio | 6.1 | macOS 10.15+ (Catalina) | VST3, CLAP | Universal binary |
| Studio One | 8.1.2 | macOS 10.15+ (Catalina) | VST3, CLAP | Intel / Apple Silicon |
| Cubase | 14.5 | macOS 10.15+ (Catalina) | VST3, CLAP | Intel / Apple Silicon |
| Ardour | 9.8 | macOS 10.15+ (Catalina) | VST3, CLAP | Universal binary |

### Windows hosts

| Host | Version | OS version | Plugin formats | Test machine |
|---|---|---|---|---|
| REAPER | 7.79 | Windows 11 23H2 (x64) | VST3, CLAP | Standard x64 PC |
| Ableton Live | 12.4.5 | Windows 11 23H2 (x64) | VST3, CLAP | Standard x64 PC |
| Bitwig Studio | 6.1 | Windows 11 23H2 (x64) | VST3, CLAP | Standard x64 PC |
| Studio One | 8.1.2 | Windows 11 23H2 (x64) | VST3, CLAP | Standard x64 PC |
| Cubase | 14.5 | Windows 11 23H2 (x64) | VST3, CLAP | Standard x64 PC |
| FL Studio | 21.2 | Windows 10+ (x64) | VST3, CLAP | Standard x64 PC |

### Linux hosts

| Host | Version | OS version | Plugin formats | Test machine |
|---|---|---|---|---|
| REAPER | 7.79 | Ubuntu 24.04 LTS (x64) | VST3, CLAP | Standard x64 PC |
| Bitwig Studio | 6.1 | Ubuntu 24.04 LTS (x64) / Flatpak | VST3, CLAP | Standard x64 PC / Flatpak sandbox |
| Ardour | 9.8 | Ubuntu 24.04 LTS (x64) | VST3, CLAP | Standard x64 PC |
| Carla | 2.5.7 | Ubuntu 24.04 LTS (x64) | VST3, CLAP | Standard x64 PC |

## Plugin SDK versions

| SDK | Version | Release date | Notes |
|---|---|---|---|
| CLAP spec | 1.2.7 | 2026-08-xx | Stable ABI, backwards compatible within 1.x |
| VST3 SDK | 3.8.1 | 2026-08-11 | MIT-licensed, Steinberg official |

## Audio specifications

| Parameter | Requirement |
|---|---|
| Sample rates | 44100, 48000, 88200, 96000 Hz (host-dependent) |
| Block sizes | Variable: host may send 0–4096 samples per processing call |
| Bit depth | f32 (32-bit float) internal processing |
| Channels | Mono and stereo input/output |

## Pass/fail test cases per host

Every host listed above must pass **all** of the following scenarios:

| # | Test case | Expected result |
|---|---|---|
| 1 | Plugin scan/discovery | Plugin appears in host plugin list |
| 2 | Instantiate | Plugin loads without crash or hang |
| 3 | Activate/deactivate | Toggle processing on/off, no audio artifacts on switch |
| 4 | Repeated start/stop | 10 consecutive start/stop cycles, no memory leak or hang |
| 5 | Zero-length block | Host sends 0-sample block, plugin returns cleanly |
| 6 | Small block size | Block size of 1–8 samples, no crash |
| 7 | Variable block sizes | Host changes block size mid-session, processing continues |
| 8 | Sample rate change | Host changes sample rate (44100→96000), plugin reinitializes |
| 9 | Automation at block boundary | Parameter automation at exact block boundaries, no gap/overlap |
| 10 | Automation at non-zero offset | Parameter automation with arbitrary sample offset within block |
| 11 | Rapid parameter changes | Max-rate parameter modulation, no click/pop |
| 12 | State save/load | Save project with plugin state, reload matches original |
| 13 | Preset load | Load preset, parameters match saved state |
| 14 | Bypass | Toggle bypass, output is silent (or unity gain per spec) |
| 15 | Editor open/close/reopen | Open plugin editor, close it, reopen — no crash or memory leak |
| 16 | Multiple instances | Load 4+ instances in same host session, all process correctly |
| 17 | Offline rendering | Render project offline at full speed without artifacts |
| 18 | Host suspend/resume | Host suspends plugin, resumes — processing continues without gap |

## Version freeze rules

1. **Do not infer** compatibility from a different version of the same host family. Each entry must be tested at the exact frozen version.
2. **No "latest"**: every version field must be an exact semantic version string.
3. **OS version**: each host entry includes the minimum tested OS version and build.
4. **Test machine**: each entry identifies the hardware platform used for testing.
5. **New versions**: if a host releases a new version after the freeze, it is added as a new entry with its own test results — it does not retroactively apply to the existing entry.

## Test environment summary

| Environment | Details |
|---|---|
| macOS test machine | Apple Silicon (M-series) + Intel Mac, macOS 15.6+ |
| Windows test machine | x64 PC, Windows 11 23H2 |
| Linux test machine | x64 PC, Ubuntu 24.04 LTS |
| CI regression | `cargo test --workspace` runs on every commit (371 tests) |
| Build target | CLAP (.so/.dylib/.dll) + VST3 (.so/.dylib/.dll) |

## Decision record

| DR | Status | Resolution |
|---|---|---|
| DR-2026-005 | **CLOSED** | This matrix defines the exact supported DAWs, versions, and platforms. See `docs/development/DECISION_STATUS-2026-09-03.md` for prior baseline acceptance. |
