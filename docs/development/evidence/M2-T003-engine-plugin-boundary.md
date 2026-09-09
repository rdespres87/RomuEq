# M2-T003 Evidence: Engine/Plugin Boundary

## Task
Prototype engine/plugin boundary to prove that the engine/plugin boundary keeps
host SDK APIs out of the core.

## Acceptance Criteria

### ✅ Core has no dependency on host SDK
- `romueq-engine` depends only on `romueq-common` (no JUCE, VST3, or AU dependencies)
- `romueq-engine/Cargo.toml` lists only `romueq-common` as a path dependency
- Host SDK types live in `romueq-engine::plugin::EngineHost`, not in the plugin traits

### ✅ Adapter holds host types
- `EngineHost` implements `PluginHost` and bridges to the engine's parameter store
  and automation ingestor
- `EnginePlugin` combines `EngineHost` with `PluginAdapter<EngineHost>`
- `PluginAdapter<H>` is generic over any `PluginHost` implementation

### ✅ Explicit lifecycle
- `PluginState` enum: `Created` → `Ready` → `Processing` → `Deactivated` → `Destroyed`
- `PluginAdapter::init()` — validates state is `Created`, calls `Plugin::init()`
- `PluginAdapter::process()` — validates state is `Ready`, calls `Plugin::process()`
- `PluginAdapter::deactivate()` — calls `Plugin::deactivate()`
- `PluginAdapter::activate()` — calls `Plugin::activate()`
- `PluginAdapter::destroy()` — calls `Plugin::cleanup()`, idempotent
- State transitions are enforced — invalid transitions are no-ops

## Files Modified

### `crates/romueq-common/src/plugin.rs` (new)
- `PluginHost` trait: `send_parameter_command`, `notify_state_change`,
  `current_sample_rate`, `current_block_size`
- `Plugin` trait: `plugin_id`, `plugin_name`, `init`, `process`, `deactivate`,
  `activate`, `cleanup`
- `PluginState` enum: `Created`, `Ready`, `Processing`, `Deactivated`, `Destroyed`
- `PluginAdapter<H>`: generic bridge type with lifecycle management
- `PluginInfo` struct: plugin metadata
- 6 tests covering lifecycle, state transitions, and plugin access

### `crates/romueq-engine/src/plugin.rs` (new)
- `EngineHost`: implements `PluginHost`, bridges to parameter store and
  automation ingestor
- `EnginePlugin`: convenience wrapper combining `EngineHost` + `PluginAdapter`
- 4 tests covering engine host functionality and full lifecycle

### `crates/romueq-plugin/src/lib.rs` (rewritten)
- Re-exports `romueq_common::plugin::*` for plugin authors
- No duplicate trait definitions

### `crates/romueq-engine/src/lib.rs` (modified)
- Added `pub mod plugin`

### `crates/romueq-common/src/lib.rs` (modified)
- Added `pub mod plugin`

### `docs/decisions/ADR-DR-2026-003-engine-plugin-boundary.md` (new)
- Architectural decision document explaining the boundary design
- Dependency graph, key types, consequences

## Test Results

```
romueq-common: 32 tests passed (6 plugin tests)
romueq-engine: 17 tests passed (4 plugin tests)
romueq-plugin: 0 tests (re-exports only)
Total: 50 tests passed, 0 failed
```

## Build Verification

```
cargo check --workspace     → exit 0
cargo fmt --all -- --check  → exit 0
cargo clippy --workspace    → exit 0
cargo test --workspace      → exit 0 (50 tests)
```

## Dependency Graph Verification

```
romueq-common (plugin module)
    ↑
    ├── romueq-engine (plugin module)
    │       ↑
    │       ├── romueq-cli
    │       ├── romueq-standalone
    │       └── romueq-integration-tests
    │
    └── romueq-plugin (re-exports romueq_common::plugin::*)
```

No circular dependencies. `romueq-plugin` depends only on `romueq-common`.
`romueq-engine` depends only on `romueq-common`.

## Conclusion

M2-T003 is complete. The engine/plugin boundary is implemented with:
- Traits in `romueq-common::plugin` (no host SDK dependency)
- `EngineHost` in `romueq-engine::plugin` (bridges to engine internals)
- `PluginAdapter<H>` generic bridge (works with any `PluginHost`)
- Explicit lifecycle with state machine enforcement
- Full test coverage (10 tests)
- Zero warnings from clippy
