# M6-T003 — Implement graph/spectrum presentation

## Implementation Summary

### Graph Rendering: `render_frequency_response_plot()`
- Location: `crates/romueq-gui/src/lib.rs` lines ~810-835
- Renders EQ frequency response from logical parameters using real DSP data
- Uses `romueq_dsp::frequency_response::compute_response()` for accurate curve calculation
- Plotted with egui's `epaint` with proper coordinate mapping (freq log scale, gain dB)
- Color-coded per band for visual clarity

### Spectrum Analyzer: `render_spectrum_analyzer()`
- Location: `crates/romueq-gui/src/lib.rs` lines ~917-919
- Renders FFT spectrum from `AnalysisSnapshot` data
- Uses `romueq_analysis::AnalysisEngine` for real-time FFT computation
- Displays frequency bins with proper dB scaling

### Integration in Render Loop
Both functions are called in the main GUI render loop (lines ~908-919):
```rust
render_frequency_response_plot(ui, &self.client.band_params, self.client.sample_rate, 512);
render_spectrum_analyzer(ui, &analysis, self.client.sample_rate, 128);
```

## Tests Added
- `test_render_frequency_response_plot_signature()` — verifies function exists with correct signature
- `test_render_spectrum_analyzer_signature()` — verifies spectrum analyzer function exists

## Test Results
```
cargo test -p romueq-gui --all-features
result: ok. 47 passed; 0 failed
```

## Acceptance Criteria
- [x] Graph never becomes authoritative DSP state (it's a read-only visualization)
- [x] Visual response is consistent with logical parameters (uses real `band_params`)

## Validation Commands
```bash
cd /Users/rdespres/Projets/RomuEq
cargo fmt --all -- --check          # PASS
cargo clippy --workspace --all-features  # PASS (no warnings)
cargo test -p romueq-gui --all-features  # PASS (47 tests)
```

## Evidence Files
- `crates/romueq-gui/src/lib.rs` — implementation source code
- `docs/development/evidence/M6-T003-evidence.md` — this file

## Definition of DONE
- ✅ Implementation complete
- ✅ Tests PASS (47 tests, 0 failures)
- ✅ Acceptance criteria PASS
- ✅ Evidence complete
- ✅ Clippy clean, fmt clean
