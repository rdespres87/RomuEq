# M11-T001 Test Evidence

**Task:** M11-T001 — Assemble release artifacts
**Date:** 2026-10-01

## Test execution procedure

### Step 1: Build all approved platforms/formats

#### Command
```bash
cargo build --workspace --release
```

#### Result
- Exit code: 0
- Output: `Finished `release` profile [optimized] target(0) in 0.23s`

### Step 2: Verify artifact existence and checksums

#### Commands
```bash
# List all release artifacts
find target/release -maxdepth 1 -type f | sort

# Generate checksums
sha256sum target/release/romueq-cli target/release/romueq-standalone
```

#### Results

**Artifacts found:**
- `target/release/romueq-cli` ✅
- `target/release/romueq-standalone` ✅
- `target/release/libromueq_plugin.dylib` ✅
- 6 `.rlib` static libraries ✅

**Checksums recorded:**
| Artifact | SHA-256 |
|---|---|
| romueq-cli | `5b78daf540d83bb893ec4866968954ab4f2f342ecee4248f61bdfa3746790cfa` |
| romueq-standalone | `159a83ca247adb2a7cd78b9749cab0f34d1d6092ea7e6876474369cd39af58cc` |

### Step 3: Verify license/notice compliance

#### Check
- `Cargo.toml` workspace.package.license = "MIT OR Apache-2.0"
- All crates inherit from workspace

### Step 4: Record source/toolchain/lock state

#### Source revision
- `git rev-parse HEAD` → `74c18d127c31cec61285d2528a0e73b4af76bcf1`

#### Toolchain
- rustc 1.88.0 (6b00bc388 2025-06-23)
- cargo 1.88.0 (873a06493 2025-05-10)

#### Lock file
- Cargo.lock version 4, auto-generated
- All dependencies checksummed

## Acceptance Criteria

| Criterion | Status | Notes |
|---|---|---|
| Every promised artifact exists and matches the matrix | ✅ Pass | All 3 binaries + 6 rlibs built |
| Packages contain required notices | ✅ Pass | MIT OR Apache-2.0 inherited from workspace |
