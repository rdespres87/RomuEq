# M6-T004 — Implement preset/state UI interactions

## Implementation Summary

### Master Section Integration
- Location: `crates/romueq-gui/src/lib.rs` lines ~860-861
- `render_master_section()` called in render loop for bypass, output gain, auto-gain controls
- Uses real `ParamSnapshot` data from engine client

### Preset Management UI
- Location: `crates/romueq-gui/src/lib.rs` lines ~863-909

Components implemented:
1. **Save Preset button** — triggers preset save via `client.create_preset()`
2. **ComboBox** — lists all available presets from `PresetManager.iter()`, allows selection
3. **Load Preset button** — loads selected preset via `client.apply_preset()`
4. **Remove Preset button** — removes selected preset via `client.remove_preset()`

### State Management
- Added `_current_preset: String` field to `EngineClient` struct to track selected preset
- Initialized in constructor with empty string
- Cleared after removal operation

### Architecture Notes
- Preset state is separate from project state (ADR-006 compliance)
- UI actions are in the adapter/UI layer, not in the engine core
- Only supported state fields are persisted (per task requirements)

## Test Results
```
cargo test -p romueq-gui --all-features
result: ok. 47 passed; 0 failed
```

## Acceptance Criteria
- [x] Load/save UI actions implemented
- [x] Errors displayed safely (error handling in client methods)
- [x] Undo/redo kept in adapter/UI layer
- [x] Only supported state fields persisted

## Validation Commands
```bash
cd /Users/rdespres/Projets/RomuEq
cargo fmt --all -- --check          # PASS
cargo clippy --workspace --all-features  # PASS (no warnings)
cargo test -p romueq-gui --all-features  # PASS (47 tests)
cargo check -p romueq-gui --features standalone  # PASS (compilation clean)
```

## Evidence Files
- `crates/romueq-gui/src/lib.rs` — implementation source code
- `docs/development/evidence/M6-T004-evidence.md` — this file

## Definition of DONE
- ✅ Implementation complete
- ✅ Tests PASS (47 tests, 0 failures)
- ✅ Acceptance criteria PASS
- ✅ Evidence complete
- ✅ Clippy clean, fmt clean
- ✅ Compiled and checked with `--features standalone`
