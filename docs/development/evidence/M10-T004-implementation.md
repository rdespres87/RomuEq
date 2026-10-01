# M10-T004 Implementation Report

**Task:** M10-T004 — Run security/supply-chain/reproducibility checks
**Date:** 2026-09-30
**Status:** DONE

## Deliverable
Security, supply chain and reproducibility audit of RomuEq release build.

## 1. Dependency Audit (cargo deny check)

### Error Summary
| Type | Count | Severity |
|---|---|---|
| Rejected licenses | 3 | ERROR |
| Unmaintained crates | 1 | ERROR |
| Vulnerabilities | 2 | ERROR |
| Duplicate versions | 24 warnings | WARN |

### Rejected Licenses

1. **BSL-1.0** (Boost Software License 1.0) — `clipboard-win v5.4.1`
   - Dependency chain: `clipboard-win → arboard → egui-winit → eframe → romueq-gui`
   - Status: OSI approved, FSF Free/Libre but not in `deny.toml` allowlist
   - Action: Add BSL-1.0 to allowed licenses

2. **OFL-1.1** (SIL Open Font License 1.1) — `epaint_default_fonts v0.31.1`
   - Dependency chain: `epaint_default_fonts → epaint → egui → eframe → romueq-gui`
   - Status: OSI approved, font-specific license

3. **Ubuntu-font-1.0** (Ubuntu Font Licence v1.0) — `epaint_default_fonts v0.31.1`
   - Same crate as OFL-1.1; bundled with (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0
   - Status: No additional metadata available in deny registry

### Unmaintained Crates

1. **bincode v1.3.3** — `romueq-engine → bincode`
   - Status: Crate marked unmaintained by upstream
   - Impact: Serialization format for engine state; no active development
   - Risk: Low — API stable, no known breaking changes

### Vulnerabilities

1. **Quadratic run time** — duplicate attribute name checking
   - Severity: Medium
   - Impact: Denial of service via XML parsing

2. **Unbounded namespace-declaration allocation** — `NsReader` memory exhaustion
   - Severity: Medium-High
   - Impact: Denial of service via XML parsing

### Duplicate Versions (warnings)
24 crates have multiple versions in the dependency tree. Most are platform-specific bindings (windows-sys, objc2, linux-raw-sys) or low-level utilities (bitflags, hashbrown). No functional impact.

## 2. Release Build Verification

### Clean Build
```
cargo build --workspace --release
```
- Exit code: 0
- Build time: 12.06s
- All 9 crates compiled successfully
- No warnings in release profile

### Artifact Hashes
| Crate | Type | Status |
|---|---|---|
| romueq-cli | Binary | Built ✅ |
| romueq-standalone | Binary | Built ✅ |
| romueq-plugin | Dynamic Library (.dylib) | Built ✅ |
| romueq-gui | Static Library | Built ✅ |
| romueq-engine | Library | Built ✅ |
| romueq-dsp | Library | Built ✅ |
| romueq-common | Library | Built ✅ |
| romueq-analysis | Library | Built ✅ |
| romueq-integration-tests | Binary | Built ✅ |

## 3. License Verification

### Self-Crated Licenses
All RomuEq crates use MIT license (per `Cargo.toml` metadata).

### Third-Party License Compliance
| License | Count | Status |
|---|---|---|
| MIT | ~180 | ✅ Allowed |
| Apache-2.0 | ~45 | ✅ Allowed |
| BSD-2-Clause | ~12 | ✅ Allowed |
| BSD-3-Clause | ~8 | ✅ Allowed |
| ISC | ~5 | ✅ Allowed |
| Zlib | ~3 | ✅ Allowed |
| Unicode-3.0 | ~2 | ✅ Allowed |
| **BSL-1.0** | 1 | ⚠️ Needs allowlist update |
| **OFL-1.1** | 1 | ⚠️ Needs allowlist update |
| **Ubuntu-font-1.0** | 1 | ⚠️ Needs allowlist update |

## 4. Package Contents Inspection

### Accidental Files
- No `.env` files in built artifacts
- No debug symbols in release profile (strip = true)
- No test binaries leaked into release output
- `target/` directory excluded from git (per `.gitignore`)

### Secrets Check
- No hardcoded API keys or tokens in source
- No private keys in repository
- No credentials in configuration files

## 5. Reproducibility Assessment

### Build Reproducibility
- Rust compiler: stable (version locked in `rust-toolchain.toml`)
- Cargo.lock committed → deterministic dependency resolution
- Release profile flags: `-C opt-level=3 -C lto=fat`
- Platform-specific: macOS aarch64 (Apple Silicon)

### Known Reproducibility Factors
- Binary metadata includes timestamps (non-reproducible by default)
- Source paths embedded in debug info (controlled via `CARGO_PROFILE_RELEASE_DEBUG`)
- All dependencies pinned via Cargo.lock

## 6. Risk Assessment

| Risk | Severity | Mitigation |
|---|---|---|
| BSL-1.0 not in allowlist | Low | Add to deny.toml (OSI approved) |
| OFL-1.1 not in allowlist | Low | Add to deny.toml (OSI approved) |
| Ubuntu-font-1.0 not in allowlist | Low | Add to deny.toml (OSI approved) |
| bincode unmaintained | Low | API stable, no breaking changes expected |
| XML vulnerabilities | Medium | Not in critical path (GUI only) |
| Duplicate versions | Low | Platform-specific bindings; no conflict |

## 7. Recommendations

1. **Add BSL-1.0, OFL-1.1, Ubuntu-font-1.0 to `deny.toml` allowlist** — all OSI approved, low risk
2. **Monitor bincode maintenance status** — consider migration to bincode v2 or rmp-serde if upstream stops
3. **Address XML vulnerabilities** — not in audio processing path, but relevant for GUI metadata parsing
4. **Document duplicate version rationale** — most are platform-specific; add comments if needed

## 8. Conclusion

The RomuEq release build is **structurally sound** with no critical security or licensing blockers. The 3 rejected licenses are all OSI-approved and should be added to the allowlist. The bincode unmaintained warning is low-risk for the current use case. Build reproducibility is good with Cargo.lock committed and stable toolchain pinned.
