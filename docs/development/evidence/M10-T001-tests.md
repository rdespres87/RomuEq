# M10-T001 Test Evidence

**Task:** M10-T001 — Freeze performance reference environment
**Date:** 2026-09-30

## Test execution procedure

### Step 1: Protocol repeatability check

#### Verify hardware specs are deterministic
```bash
cd /Users/rdespres/Projets/RomuEq
sysctl -n machdep.cpu.brand_string 2>&1
sysctl -n hw.ncpu 2>&1
sysctl -n hw.memsize 2>&1
```
**Expected:** Output matches reference environment (Apple M5 Pro, 18 cores, 64 GB).
**Actual:** ✅ PASS — Apple M5 Pro, 18 cores, 64 GB (68719476736 bytes).

#### Verify OS version is deterministic
```bash
sw_vers 2>&1 | head -5
rustc --version 2>&1
```
**Expected:** macOS version and rustc version match specification.
**Actual:** ✅ PASS — macOS 26.6.2, rustc version matches `rust-version = "1.88"`.

#### Verify baseline commit is stable
```bash
git log --oneline -1 afc3ba1 2>&1
cargo test --workspace 2>&1 | tail -5
```
**Expected:** Baseline commit exists, all tests pass.
**Actual:** ✅ PASS — `afc3ba1` is the latest commit, 371 tests passing.

#### Verify no benches/ directory exists yet
```bash
ls -la benches/ 2>&1
grep -c "criterion" Cargo.toml 2>&1
```
**Expected:** No benches/ directory, no criterion dependency.
**Actual:** ✅ PASS — `benches/` does not exist, `criterion` not in Cargo.toml.

### Step 2: Protocol parameter verification

#### Verify all 6 test scenarios are fully specified
| # | Sample Rate | Channels | Block Size | Active Bands | Automation | Analysis |
|---|---|---|---|---|---|---|
| 1 | 48 kHz | Stereo | 64 | 8 | 0.1 Hz LFO | Disabled |
| 2 | 48 kHz | Stereo | 256 | 8 | 0.1 Hz LFO | Disabled |
| 3 | 96 kHz | Stereo | 64 | 8 | 0.1 Hz LFO | Disabled |
| 4 | 48 kHz | Mono | 64 | 8 | 0.1 Hz LFO | Disabled |
| 5 | 48 kHz | Stereo | 64 | 8 | 10 Hz LFO | Disabled |
| 6 | 48 kHz | Stereo | 64 | 8 | None | 4 instances |

**Expected:** All 6 scenarios have complete parameter specifications.
**Actual:** ✅ PASS — all parameters documented in M10-T001-implementation.md.

### Step 3: Environment immutability check

#### Verify frozen parameters
| Parameter | Frozen Value | Change Condition |
|---|---|---|
| CPU model | Apple M5 Pro | Version to v2 |
| RAM | 64 GB | Version to v2 |
| OS | macOS 26.6.2 | Version to v2 |
| Compiler | rustc 1.88+ | Version to v2 |
| Build profile | `--release` (lto=fat) | Document as regression |
| Baseline commit | `afc3ba1` | New baseline via DR |

**Expected:** All parameters have documented change conditions.
**Actual:** ✅ PASS — immutability rules defined.

## Repeatability verification

### How to reproduce this benchmark
```bash
# 1. Verify environment
sysctl -n machdep.cpu.brand_string
sw_vers
rustc --version

# 2. Verify baseline
cd /Users/rdespres/Projets/RomuEq
git checkout afc3ba1
cargo test --workspace

# 3. Run benchmark scenarios (M10-T002 will implement)
#    cargo bench --workspace  # once criterion is added

# 4. Record results with full environment metadata
```

### Expected output format for future benchmarks
```
M10 Benchmark Report v1.0
Reference: Apple M5 Pro / macOS 26.6.2 / rustc 1.88
Baseline: afc3ba1

Scenario 1: 48kHz stereo 64-block 8-bands
  CPU: X.X%
  Process time p50: X.X us
  Process time p99: X.X us
  Allocations: 0 (hot path)
```

## Acceptance criteria verification

| Criterion | Status | Evidence |
|---|---|---|
| Reference environment and benchmark protocol are immutable for comparison unless versioned | ✅ PASS | Environment frozen, immutability rules documented, baseline commit recorded |

## Decision log

| Decision | Status | Reference |
|---|---|---|
| DR-2026-006: Performance budget and reference machine | ACCEPTED (provisional) | `docs/development/PERFORMANCE_BUDGET.md` |
| Reference machine: Apple M5 Pro, 64 GB | ACCEPTED | `sysctl` output captured |
| Baseline commit: afc3ba1 | ACCEPTED | M9 gate PASS, all tests pass |
| No criterion benchmark yet | ACCEPTED | benches/ does not exist, M10-T002 will create it |
