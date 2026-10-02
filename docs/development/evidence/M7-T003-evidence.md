# M7-T003 — Implement plugin state/automation mapping

## Implementation Summary

### Automation System: `romueq-engine/src/automation.rs`
- **AutomationIngestor**: Receives raw `AutomationEvent` from host, validates offsets, enforces deterministic ordering by `(sample_offset, sequence)`, deduplicates events targeting same parameter at same offset.
- **AutomationProcessor**: Applies automation events to `ParameterStore`, supports sample-accurate value lookup with interpolation modes (Hold, Linear).
- **AutomationValueProvider**: Interpolates automation values at arbitrary sample positions.

### State Persistence: `romueq-engine/src/parameter_store.rs`
- Stores per-parameter automation points `(sample_offset, value)` tuples.
- Thread-safe via `Arc<Mutex<...>>`.
- Supports snapshot/restore for preset operations.

### Plugin State Integration
- **CLAP plugin**: `RomuEqPlugin::save_state()` / `load_state()` use `EngineLifecycle::snapshot()` and state serialization via `romueq-engine`.
- **VST3 plugin**: `RomuEqAudioProcessor` wraps `EngineLifecycle` with `Arc<Mutex<...>>`, supports reset and parameter mapping.

## Evidence

### Unit Tests
```bash
$ cargo test -p romueq-engine automation::tests --all-features
test automation::tests::test_full_pipeline_deterministic_order ... ok
test automation::tests::test_full_pipeline_large_block ... ok
test automation::tests::test_full_pipeline_malformed_events_no_corruption ... ok
test automation::tests::test_ingestor_default_is_hold_mode ... ok
test automation::tests::test_ingestor_drops_out_of_range_events ... ok
test automation::tests::test_ingestor_pushes_and_processes_block ... ok
test automation::tests::test_ingestor_reset_clears_pending ... ok
test automation::tests::test_ingestor_value_provider ... ok
test automation::tests::test_processor_applies_batch_to_store ... ok
test automation::tests::test_processor_applies_empty_batch ... ok
test automation::tests::test_processor_get_unknown_param ... ok
test automation::tests::test_processor_overwrites_values ... ok
test automation::tests::test_processor_store_access ... ok
```

### Automation Mapping Coverage
- Parameter ID mapping: `ParamId` → automation event targets
- Sample-accurate interpolation: linear and hold modes validated
- Block-rate validation: out-of-range events silently dropped
- Deduplication: same param + same offset → highest sequence wins
- Full pipeline: ingestor → batch → processor → parameter store

### CI Validation
```bash
$ cargo fmt --all -- --check          # OK
$ cargo clippy --workspace ...         # CLEAN (no warnings)
$ cargo test --workspace --all-features # 396 passing
```

## Acceptance Criteria
- [x] Automation events validated against block length
- [x] Deterministic ordering by `(sample_offset, sequence)`
- [x] Deduplication of duplicate events at same offset
- [x] Interpolation modes (Hold, Linear) implemented and tested
- [x] Thread-safe parameter store with Arc<Mutex<...>>
- [x] State snapshot/restore for presets
- [x] No out-of-scope architectural changes
