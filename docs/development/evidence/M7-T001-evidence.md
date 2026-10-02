# M7-T001 — Implement CLAP adapter

## Implementation Summary

### CLAP Plugin: `romueq-plugin`
- Location: `crates/romueq-plugin/src/lib.rs`
- Framework: clack-plugin v0.1.x, clap-sys v0.5
- Library type: cdylib + rlib

### Architecture
- **RomuEqShared**: Thread-safe engine reference (Arc<Mutex<EngineLifecycle>>)
- **RomuEqMainThread**: Main thread marker with engine access
- **RomuEqAudioProcessor**: Bridges CLAP audio processing with RomuEq engine
- **RomuEqState**: CLAP State extension for save/load
- **RomuEqPlugin**: Plugin marker tying thread model together

### Key Features
- Audio processing via `process()` with multi-channel support
- CLAP State extension (save/load via InputStream/OutputStream)
- Engine lifecycle management (prepare, activate, deactivate)
- DefaultPluginFactory for CLAP host registration

### Test Results
- 5 tests passing, 0 failed
- `cargo clippy --workspace` clean
- `cargo fmt --all -- --check` clean

### Commits
- `65c84a8` feat(romueq-plugin): implement CLAP plugin with clack-plugin adapter
- `361bf3b` feat(romueq-plugin): implement audio processor with multi-channel support
- `8d262ee` feat(romueq-plugin): share EngineLifecycle via RomuEqShared for state persistence
- `a01898e` feat(romueq-plugin): implement CLAP state extension (save/load)

## Acceptance Criteria
- [x] CLAP plugin builds as cdylib
- [x] Audio processor bridges CLAP to RomuEq engine
- [x] State persistence via CLAP extension
- [x] All workspace tests pass (5/5)
- [x] No clippy warnings
- [x] Format clean

## Gate Impact: M7
