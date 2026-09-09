# M2-T004 Evidence: Validation

## Task
M2-T004 — Prototype audio buffer and lifecycle contract.

## Validation Commands

```bash
cd /Users/rdespres/Projets/RomuEq

# Format check
cargo fmt --all -- --check

# Compilation check
cargo check --workspace

# Lint check
cargo clippy --workspace

# Test suite
cargo test --workspace --all-features
```

## Results

### Format check
```
cargo fmt --all -- --check → exit 0
```

### Compilation check
```
cargo check --workspace → exit 0
```

### Lint check
```
cargo clippy --workspace → exit 0
```

### Test suite
```
romueq-common: 62 tests passed (29 audio_buffer tests)
romueq-engine: 17 tests passed
romueq-plugin: 0 tests (re-exports only)
romueq-integration-tests: 1 test passed
Total: 80 tests passed, 0 failed
```

## Acceptance Criteria Verification

| Criterion | Status |
|-----------|--------|
| Buffer contract is explicit | ✅ Planar f32, MAX_CHANNELS=8, MAX_BLOCK_SIZE=8192 |
| Invalid host behaviour is safe and deterministic | ✅ Panics with descriptive messages |
| prepare/reprepare/reset semantics are testable | ✅ 29 tests covering all lifecycle states |

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Panic on invalid host | Host SDK layer wraps in `catch_unwind` |
| Planar vs interleaved conversion | Conversion at host SDK boundary (M3+) |
| `Vec<Vec<f32>>` indirection | Flat buffer with stride indexing (future) |

## Conclusion

M2-T004 validation is complete. All acceptance criteria are met, all tests
pass, and the ADR documents the design decisions and consequences.
