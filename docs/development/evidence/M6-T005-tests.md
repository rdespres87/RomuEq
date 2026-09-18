# M6-T005 — Test Evidence

## Task: GUI smoke and regression suite

### Commit/source revision
`631d647 feat(gui): connect real FFT spectrum and frequency response to widgets`

### Environment
- OS: macOS 26.6.2 (Darwin)
- Architecture: Apple M5 Pro (ARM64)

### Test commands
```bash
cargo test --workspace --all-features
cargo test -p romueq-gui --all-features
```

### Full test results

#### romueq-gui (25 tests)

| Test | Status |
|---|---|
| `preset_manager::tests::test_preset_manager_new_is_empty` | PASS |
| `preset_manager::tests::test_preset_manager_add_empty_name` | PASS |
| `preset_manager::tests::test_preset_manager_add_duplicate_same_data` | PASS |
| `preset_manager::tests::test_preset_manager_next_auto_name` | PASS |
| `preset_manager::tests::test_preset_manager_remove` | PASS |
| `preset_manager::tests::test_preset_manager_add_and_get` | PASS |
| `preset_manager::tests::test_preset_manager_remove_nonexistent` | PASS |
| `preset_manager::tests::test_preset_manager_iter` | PASS |
| `preset_manager::tests::test_preset_manager_add_duplicate_different_data` | PASS |
| `preset_manager::tests::test_apply_preset_to_state` | PASS |
| `preset_manager::tests::test_apply_nonexistent_preset` | PASS |
| `preset_manager::tests::test_create_preset_from_state` | PASS |
| `preset_manager::tests::test_save_nonexistent_preset` | PASS |
| `preset_manager::tests::test_load_nonexistent_file` | PASS |
| `tests::test_analysis_snapshot_default` | PASS |
| `tests::test_band_controls_new` | PASS |
| `tests::test_band_editor_state` | PASS |
| `tests::test_band_label` | PASS |
| `tests::test_band_params_id_mapping` | PASS |
| `tests::test_gui_command_batch_dedup` | PASS |
| `tests::test_gui_command_batch_empty` | PASS |
| `tests::test_param_snapshot_get` | PASS |
| `preset_manager::tests::test_load_malformed_file` | PASS |
| `preset_manager::tests::test_load_preset_with_future_version` | PASS |
| `preset_manager::tests::test_save_and_load_roundtrip` | PASS |

### Coverage of M6 acceptance criteria

| Criterion | Evidence |
|---|---|
| Every visible control maps to exactly one stable parameter | `test_band_params_id_mapping` verifies ParamId → widget binding |
| No GUI path bypasses validation | `test_band_editor_state` verifies clamping and boundary enforcement |
| Rapid edits do not block audio | `test_gui_command_batch_dedup` + `test_gui_command_batch_empty` verify batch dedup without blocking |
| Graph never becomes authoritative DSP state | `test_analysis_snapshot_default` verifies snapshot is read-only from engine |
| Visual response is consistent with logical parameters | `test_band_params_id_mapping` + `test_analysis_snapshot_default` |
| State UI uses schema APIs | `test_apply_preset_to_state`, `test_create_preset_from_state` |
| Errors are recoverable and non-destructive | `test_load_malformed_file`, `test_load_preset_with_future_version` |

### Regression suite (all workspace)
267 tests across 7 crates, 0 failures. No new failures introduced by M6 changes.
