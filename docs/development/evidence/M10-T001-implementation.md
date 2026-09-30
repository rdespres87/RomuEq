# M10-T001 Implementation Report

**Task:** M10-T001 — Freeze performance reference environment
**Date:** 2026-09-30
**Status:** DONE

## Deliverable
Frozen the performance reference environment and measurement protocol for all future M10/M11 benchmarks.

## Reference environment (frozen)

### Hardware
| Parameter | Value |
|---|---|
| CPU | Apple M5 Pro |
| Cores | 18 (10 performance + 8 efficiency) |
| RAM | 64 GB |
| Architecture | ARM64 (AArch64) |

### Software
| Parameter | Value |
|---|---|
| OS | macOS 26.6.2 (Build 25G83) |
| Compiler | rustc 1.88+ (per `rust-version = "1.88"` in Cargo.toml) |
| Build profile | `--release` (opt-level=3, lto=fat) |
| Rust edition | 2024 |

### Power and thermal
| Parameter | Value |
|---|---|
| Power mode | System default (adaptive) |
| Thermal throttling | Not controlled (desktop/laptop ambient) |

### Benchmark protocol
| Parameter | Value |
|---|---|
| Sample rate | 48000 Hz (primary), 96000 Hz (secondary) |
| Block sizes | 64, 128, 256 samples (primary); 1, 3, 512 (edge) |
| Channel count | Mono + Stereo (both required) |
| Active bands | 0, 4, 8 (worst-case) |
| Instance count | 1 (primary), 4, 8 (scaling) |
| Warm-up duration | 2 seconds (32k samples at 48kHz) |
| Measurement duration | 5 seconds (240k samples at 48kHz) |
| Automation load | Sine-wave LFO sweep across all parameters at 0.1 Hz |
| Analysis enabled | Both enabled and disabled (two runs) |

### Measurement metrics
| Metric | Method |
|---|---|
| CPU usage | macOS `powermetrics` or `top` over measurement window |
| Process time | `std::time::Instant` in engine process() per block |
| Memory allocation | `jemalloc` profiling or `malloc_zone_statistics` |
| Allocation count | Zero in hot path (verified via test harness) |

### Test matrix (6 scenarios from PERFORMANCE_BUDGET.md)
| # | Sample Rate | Channels | Block Size | Active Bands | Automation | Analysis |
|---|---|---|---|---|---|---|
| 1 | 48 kHz | Stereo | 64 | 8 | 0.1 Hz LFO | Disabled |
| 2 | 48 kHz | Stereo | 256 | 8 | 0.1 Hz LFO | Disabled |
| 3 | 96 kHz | Stereo | 64 | 8 | 0.1 Hz LFO | Disabled |
| 4 | 48 kHz | Mono | 64 | 8 | 0.1 Hz LFO | Disabled |
| 5 | 48 kHz | Stereo | 64 | 8 | 10 Hz LFO (worst-case) | Disabled |
| 6 | 48 kHz | Stereo | 64 | 8 | None | 4 instances |

## Baseline commit
**Commit:** `afc3ba1` (M9-T005 compatibility gate)
**Rationale:** M9 is complete; this is the last stable commit before M10 benchmarks.

## Decision
**DR-2026-006:** Performance budget and reference machine — ACCEPTED (provisional).
The provisional target from PERFORMANCE_BUDGET.md is **<5% CPU for stereo 48 kHz processing** on this reference machine.

## Files created/modified

| File | Action | Description |
|---|---|---|
| `docs/development/evidence/M10-T001-implementation.md` | Created | This file — reference environment freeze |
| `docs/development/evidence/M10-T001-tests.md` | Created | Protocol repeatability check |
| `docs/development/evidence/M10-T001-evidence.md` | Created | Evidence summary and traceability |

## Acceptance criteria

| Criterion | Status | Notes |
|---|---|---|
| Reference environment and benchmark protocol are immutable for comparison unless versioned | ✅ PASS | Environment frozen, baseline commit recorded, protocol documented |

## Notes

- **No `benches/` directory:** The project has no criterion benchmark harness yet. M10-T002 will create one.
- **No `criterion` dependency:** Not in Cargo.toml. M10-T002 will add it.
- **Provisional target:** `<5% CPU stereo 48kHz` is an engineering target, not a release claim.
- **Future immutability:** Any change to this environment must be versioned (e.g., `v2`), not in-place.

## Confidence scoring

| Criterion | Score | Notes |
|---|---|---|
| Hardware specification | High | Exact model, cores, RAM from `sysctl` |
| Software specification | High | OS version, compiler version, build profile documented |
| Protocol repeatability | High | 6 scenarios with exact parameters defined |
| Baseline commit | High | `afc3ba1` — last M9 commit, fully tested |

**Overall confidence: HIGH** — reference environment frozen and documented.
