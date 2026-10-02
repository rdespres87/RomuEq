# M6-T001 — Freeze GUI framework and lifecycle contract

## Implementation Summary

### GUI Framework: egui
- Location: `crates/romueq-gui/src/lib.rs`
- Framework: egui 0.31 (pinned in Cargo.toml)
- Lifecycle contract: `EngineClient` wraps `Option<EngineLifecycle>` with proper initialization/cleanup
- Standalone runner: `cargo run --features standalone` launches egui window

### Architecture
- `EngineClient` struct manages engine lifecycle (creation, parameter snapshot, command batching)
- GuiCommandBatch for atomic parameter updates from UI to engine
- `render_*()` functions for modular UI sections (band controls, master section, graphs)

### Test Coverage
- `test_render_master_section_signature()` — verifies function signature
- `test_analysis_snapshot_dropped_flag()` — verifies analysis state tracking
- 47 romueq-gui tests passing

## Validation Commands
```bash
cargo fmt --all -- --check          # PASS
cargo clippy --workspace --all-targets --all-features  # PASS (clean)
cargo test -p romueq-gui --all-features  # PASS (47 tests)
```

## Definition of DONE
- ✅ egui framework frozen at 0.31
- ✅ Lifecycle contract implemented (EngineClient)
- ✅ Tests PASS (47 tests, 0 failures)
- ✅ Clippy clean, fmt clean
