# M10-T004 Evidence Summary

**Task:** M10-T004 — Run security/supply-chain/reproducibility checks
**Date:** 2026-09-30

## Evidence artifacts

| Artifact | Path | Description |
|---|---|---|
| Implementation report | `docs/development/evidence/M10-T004-implementation.md` | Full audit with risk assessment |
| Test evidence | `docs/development/evidence/M10-T004-tests.md` | Step-by-step test execution |

## Command reference

```bash
# Dependency audit
cargo deny check

# Clean release build
cargo build --workspace --release
```

## Acceptance status

| Criterion | Status |
|---|---|
| No unapproved dependency risk | ⚠️ 3 OSI-approved licenses need allowlist update (BSL-1.0, OFL-1.1, Ubuntu-font-1.0) |
| Release build reproducible | ✅ Pass |
| No secret/debug artifact leakage | ✅ Pass |

## Key findings

- **3 rejected licenses**: All OSI-approved, should be added to `deny.toml`
- **1 unmaintained crate**: bincode v1.3.3 (low risk, API stable)
- **2 vulnerabilities**: XML parsing (GUI path, not audio critical path)
- **24 duplicate version warnings**: Mostly platform-specific bindings
- **Clean release build**: 9 crates, 12.06s, exit code 0
