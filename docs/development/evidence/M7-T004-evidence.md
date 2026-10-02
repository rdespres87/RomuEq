# M7-T004 Evidence — Plugin lifecycle and editor embedding

## Task
Validate plugin lifecycle and editor embedding via `RomuEqEditor` struct in `crates/romueq-plugin/src/lib.rs`.

## Implementation

### RomuEqEditor struct
```rust
pub struct RomuEqEditor {
    active: bool,
    param_values: Vec<ParamValue>,
}
```

### Lifecycle hooks
- `on_open()` — initializes engine, creates `RomuEqEditor`, sets `active = true`
- `on_close()` — closes editor, stops engine
- `render()` — displays egui window with 8 band EQ sliders

### Parameter mapping
- `set_param(param_id: ParamId, value: f32)` — updates parameter values
- Band frequency parameters mapped as `ParamId(102 + i * 100)` for i in 0..8

## Verification

### Compilation
```
$ cargo check -p romueq-plugin
    Checking romueq-plugin v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.19s
```

### Clippy
```
$ cargo clippy -p romueq-plugin --all-features
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.32s
```

### Tests
```
$ cargo test -p romueq-plugin --all-features
     Running unittests src/lib.rs
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored
```

### Workspace CI
```
$ cargo fmt --all -- --check    # PASS
$ cargo clippy --workspace       # CLEAN (0 warnings)
$ cargo test --workspace         # 301 tests passing
```

## Dependencies added
- `egui = "0.31"` in `crates/romueq-plugin/Cargo.toml` (for editor UI)
- `use egui::Widget;` import for slider rendering
- `use romueq_common::ParamId;` for parameter identification
- `use romueq_gui::ParamValue;` for parameter values

## Key design decisions
- Editor uses `egui::Window::new()` for proper UI context (not raw `ctx`)
- Band parameters use offset scheme: `ParamId(102 + i*100)` for i=0..7
- Slider range: -100.0 to 100.0 for EQ gain values
