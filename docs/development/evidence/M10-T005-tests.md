# M10-T005 Test Evidence

**Task:** M10-T005 — Run M10 hardening gate
**Date:** 2026-10-01

## Test execution procedure

### Step 1: Run complete regression suite

#### Commands
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

#### Results

| Command | Status | Details |
|---|---|---|
| `cargo fmt --all -- --check` | ✅ PASS | exit 0 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | ✅ PASS | exit 0, 9 crates checked |
| `cargo test --workspace --all-features` | ✅ PASS | 297 tests, 0 failed |
| `cargo deny check` | ⚠️ Expected | 3 license rejections (OSI-approved), 1 unmaintained, 2 vulnerabilities |

### Step 2: Verify all P0 requirements have evidence

#### M10-T001 (Freeze performance reference environment)
- ✅ `docs/development/evidence/M10-T001-implementation.md` — present
- ✅ `docs/development/evidence/M10-T001-tests.md` — present
- ✅ `docs/development/evidence/M10-T001-evidence.md` — present

#### M10-T002 (Measure CPU/memory/allocation behavior)
- ✅ `docs/development/evidence/M10-T002-implementation.md` — present
- ✅ `docs/development/evidence/M10-T002-tests.md` — present
- ✅ `docs/development/evidence/M10-T002-evidence.md` — present

#### M10-T003 (Run fuzz/property/stress suite)
- ✅ `docs/development/evidence/M10-T003-implementation.md` — present
- ✅ `docs/development/evidence/M10-T003-tests.md` — present
- ✅ `docs/development/evidence/M10-T003-evidence.md` — present

#### M10-T004 (Run security/supply-chain/reproducibility checks)
- ✅ `docs/development/evidence/M10-T004-implementation.md` — present
- ✅ `docs/development/evidence/M10-T004-tests.md` — present
- ✅ `docs/development/evidence/M10-T004-evidence.md` — present

### Step 3: Review risk register

#### Risk acceptance summary
| Risk | Severity | Status |
|---|---|---|
| BSL-1.0 license | Low | Accepted |
| OFL-1.1 license | Low | Accepted |
| Ubuntu-font-1.0 license | Low | Accepted |
| bincode unmaintained | Low | Accepted |
| XML quadratic runtime | Medium | Accepted |
| XML unbounded allocation | Medium | Accepted |
| Duplicate versions (24) | Low | Accepted |

All 7 risks are P2 or below. No P0/P1 open.

### Step 4: Freeze release candidate baseline

#### Baseline commit: `4d2e094`
- All M10 tasks done
- All tests passing (297/297)
- Clippy clean
- Format clean

## Acceptance Criteria

| Criterion | Status | Notes |
|---|---|---|
| M10 PASS | ✅ Pass | All 5 sub-tasks done, all tests pass |
| Release candidate baseline frozen | ✅ Pass | Commit `4d2e094` |
| All P0 risks closed or accepted | ✅ Pass | 0 P0 open, 7 P2 accepted |
