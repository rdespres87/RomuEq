# RomuEq Review Progress

## Phase 1 — Engine Initialization, Spectrum, Params Sync
**Commit:** (pending) `feat(romueq-gui): initialize engine, populate spectrum, sync band params`
**Date:** 2026-10-05
**Status:** In progress

### Issues Found

| ID | Severity | Description | File | Status |
|----|----------|-------------|------|--------|
| S1 | High | `StandaloneRunner::new()` ne callait pas `client.prepare()` — l'engine restait uninitialized | `lib.rs:813` | Fixed |
| S2 | High | `apply_params_from_batch()` ne mettait à jour que le gain — filter_type, frequency, q, slope ignorés | `lib.rs:995` | Fixed |
| S3 | Medium | `analysis_snapshot()` retournait spectrum_data vide — pas de visualisation | `lib.rs:1255` | Fixed |
| S4 | Medium | Double `CentralPanel` dans `main.rs` écrasait l'UI du `StandaloneRunner` | `main.rs` | Fixed |
| S5 | Low | `apply_commands()` ne synchronisait pas le snapshot après application des commandes | `lib.rs:1355` | Fixed |

### Fixes Applied

1. **Engine initialization** — `client.prepare(48000.0, 512, 2)` ajouté dans `StandaloneRunner::new()`
2. **Band params sync** — `apply_params_from_batch()` met à jour enabled, filter_type, frequency_hz, gain_db, q, slope
3. **Spectrum generation** — `analysis_snapshot()` génère 128 bins depuis les band_params (EQ response synthétique)
4. **Snapshot sync** — `apply_commands()` met à jour `self.snapshot` après application des commandes
5. **Double panel** — suppression du `CentralPanel` dupliqué dans `main.rs`
6. **Spectrum processor** — `_spectrum_processor` et `_spectrum_frame_number` ajoutés à `EngineClient` pour Phase 6

### CI Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS
- `cargo test --workspace --all-features` — 387 tests PASS

### Evidence

- Build standalone: `cargo run --package romueq-standalone --features "romueq-gui/standalone"`
- Tests: `cargo test --workspace --all-features` (387 passed)
- Clippy: `cargo clippy --workspace --all-targets --all-features -- -D warnings` (clean)

### Remaining Work

- **Phase 2** — Real-time spectrum from audio input (requires Phase 6 audio backend)
- **Phase 3** — Interactive widgets (sliders, knobs) — widgets exist but need drag interaction
- **Phase 4** — Preset save/load from disk
- **Phase 5** — Band-to-engine synchronization (partially done via apply_params_from_batch)
- **Phase 6** — Audio backend with `cpal`

### Notes

- Spectrum actuel est synthétique (EQ response calculée depuis band_params) — pas de vrai FFT audio
- Phase 6 (cpal) nécessaire pour spectrum réel
- M6-T001 à M6-T005 déjà DONE dans TASKS.md
