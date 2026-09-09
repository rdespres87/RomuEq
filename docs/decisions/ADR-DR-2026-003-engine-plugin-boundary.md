# ADR-DR-2026-003: Engine/Plugin Boundary

## Status
Accepted

## Context
RomuEq must support multiple host SDKs (JUCE, VST3, AU, etc.) while keeping
the core engine and plugin logic host-agnostic. The current codebase has no
defined boundary between the engine and plugins — `romueq-engine` and
`romueq-plugin` are both stubs.

We need a clear contract that:
1. Keeps host SDK types out of the engine core.
2. Allows plugin authors to implement audio processing without knowing the host.
3. Provides a well-defined lifecycle for plugin initialisation, processing,
   and cleanup.

## Decision
We define the plugin boundary in `romueq-common::plugin` (not in `romueq-plugin`)
to avoid a circular dependency: `romueq-engine` needs `PluginHost` to implement
the host side, and `romueq-plugin` needs `Plugin` to define the plugin side.
If both traits lived in `romueq-plugin`, then `romueq-engine` would depend on
`romueq-plugin`, and `romueq-plugin` would depend on `romueq-engine` (for
`PluginAdapter`), creating a cycle.

By placing the traits in `romueq-common`, both crates can depend on them
without circularity.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Host SDK (JUCE / VST3 / AU)                            │
│  ┌───────────────────────────────────────────────────┐  │
│  │  Engine (romueq-engine)                           │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  EngineHost (implements PluginHost)         │  │  │
│  │  │  PluginAdapter<EngineHost>                  │  │  │
│  │  │  EnginePlugin (convenience wrapper)         │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  │         ▲                                          │  │
│  │         │ calls                                    │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  Plugin (trait)                             │  │  │
│  │  │  PluginHost (trait)                         │  │  │
│  │  │  PluginState (enum)                         │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  │         ▲                                          │  │
│  │         │ trait                                    │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  Plugin (trait)                             │  │  │
│  │  │  PluginAdapter<H>                           │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  │         ▲                                          │  │
│  │         │ implements                             │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  Plugin (trait)                             │  │  │
│  │  │  (user-implemented)                         │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Key types

- **`Plugin`** (in `romueq-common::plugin`): trait that plugin authors implement.
  Methods: `plugin_id`, `plugin_name`, `init`, `process`, `deactivate`,
  `activate`, `cleanup`.

- **`PluginHost`** (in `romueq-common::plugin`): trait that the host/engine
  implements. Methods: `send_parameter_command`, `notify_state_change`,
  `current_sample_rate`, `current_block_size`.

- **`PluginAdapter<H>`** (in `romueq-common::plugin`): generic bridge type
  that owns a `PluginHost` and a `Box<dyn Plugin>`. Manages lifecycle state
  transitions.

- **`EngineHost`** (in `romueq-engine::plugin`): implements `PluginHost`,
  bridges to the engine's parameter store and automation ingestor.

- **`EnginePlugin`** (in `romueq-engine::plugin`): convenience wrapper that
  combines `EngineHost` with `PluginAdapter<EngineHost>`.

- **`PluginState`** (in `romueq-common::plugin`): enum tracking lifecycle
  state (`Created`, `Ready`, `Processing`, `Deactivated`, `Destroyed`).

### Dependency graph

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

## Consequences

### Positive
- **No circular dependencies**: traits in `romueq-common`, implementations
  in `romueq-engine`, re-exports in `romueq-plugin`.
- **Host SDK isolation**: host-specific types (JUCE `AudioProcessor`, VST3
  `AudioEffect`, etc.) live only in `romueq-engine`, not in the plugin
  trait or the engine core.
- **Testability**: `PluginHost` is a trait, so tests can use a `StubHost`
  double. `Plugin` is a trait, so tests can use a `StubPlugin` double.
- **Future extensibility**: new host SDKs only need to implement `PluginHost`
  and create an `EnginePlugin` (or equivalent).

### Negative
- **Trait object overhead**: `Box<dyn Plugin>` introduces vtable dispatch.
  For audio-critical paths, this may need to be replaced with generics or
  code generation in the future.
- **`RefCell` in tests**: `StubHost` uses `RefCell` for mutable state because
  `PluginHost` methods take `&self`. This is acceptable for tests but
  should not leak into production code.

## References
- `crates/romueq-common/src/plugin.rs` — trait definitions
- `crates/romueq-engine/src/plugin.rs` — `EngineHost` and `EnginePlugin`
- `crates/romueq-plugin/src/lib.rs` — re-exports
- `tasks.json` — M2-T003
