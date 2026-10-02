# M6-T002 — Implement parameter editor and band controls

## Implementation Summary

### Band Controls UI
- Location: `crates/romueq-gui/src/lib.rs` — `render_band_controls()`
- egui widgets for each EQ band: frequency, gain, Q factor, bypass toggle
- Real-time parameter editing via `BandControls` model
- Stable `ParamId` bindings for each control (per ADR-006)

### Band Parameters
- Location: `crates/romueq-common/src/parameter.rs` — `BandParams` struct
- 10-band EQ with frequency spacing from 20Hz to 20kHz
- `frequency_hz()` returns `ParamId` for stable ID binding

### Band Editor Model
- Single-param editing model: `BandEditor` keeps only the currently edited parameter
- Previous parameter restored when user switches focus
- Prevents partial edits from leaking into engine state

### Integration
- Band controls wired to `EngineClient.pending_commands` for atomic updates
- `BandControls::new().params(i)` returns band parameter struct with ParamId

## Test Coverage
- `test_band_params_default_values()` — verifies default BandParams
- `test_band_controls_params()` — verifies control param access
- `test_band_params_frequency_hz_positive()` — verifies positive frequency IDs
- `test_band_editor_multiple_bands()` — verifies single-param editing model

## Validation Commands
```bash
cargo fmt --all -- --check          # PASS
cargo clippy --workspace --all-targets --all-features  # PASS (clean)
cargo test -p romueq-gui --all-features  # PASS (47 tests)
```

## Definition of DONE
- ✅ Band controls UI implemented
- ✅ ParamId bindings stable (ADR-006 compliant)
- ✅ BandEditor single-param editing model
- ✅ Tests PASS (47 tests, 0 failures)
- ✅ Clippy clean, fmt clean
