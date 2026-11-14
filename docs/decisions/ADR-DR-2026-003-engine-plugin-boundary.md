# ADR-DR-2026-003: Engine/Plugin Boundary

## Status
Accepted

## Context
Equaform must support multiple host SDKs (JUCE, VST3, AU, etc.) while keeping
the core engine and plugin logic host-agnostic. The current codebase has no
defined boundary between the engine and plugins — `equaform-engine` and
`equaform-plugin` are both stubs.

We need a clear contract that:
1. Keeps host SDK types out of the engine core.
2. Allows plugin authors to implement audio processing without knowing the host.
3. Provides a well-defined lifecycle for plugin initialisation, processing,
   and cleanup.

## Decision
We define the plugin boundary in `equaform-common::plugin` (not in `equaform-plugin`)
to avoid a circular dependency: `equaform-engine` needs `PluginHost` to implement
the host side, and `equaform-plugin` needs `Plugin` to define the plugin side.
If both traits lived in `equaform-plugin`, then `equaform-engine` would depend on
`equaform-plugin`, and `equaform-plugin` would depend on `equaform-engine` (for
`PluginAdapter`), creating a cycle.

By placing the traits in `equaform-common`, both crates can depend on them
without circularity.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Host SDK (JUCE / VST3 / AU)                            │
│  ┌───────────────────────────────────────────────────┐  │
│  │  Engine (equaform-engine)                           │  │
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

- **`Plugin`** (in `equaform-common::plugin`): trait that plugin authors implement.
  Methods: `plugin_id`, `plugin_name`, `init`, `process`, `deactivate`,
  `activate`, `cleanup`.

- **`PluginHost`** (in `equaform-common::plugin`): trait that the host/engine
  implements. Methods: `send_parameter_command`, `notify_state_change`,
  `current_sample_rate`, `current_block_size`.

- **`PluginAdapter<H>`** (in `equaform-common::plugin`): generic bridge type
  that owns a `PluginHost` and a `Box<dyn Plugin>`. Manages lifecycle state
  transitions.

- **`EngineHost`** (in `equaform-engine::plugin`): implements `PluginHost`,
  bridges to the engine's parameter store and automation ingestor.

- **`EnginePlugin`** (in `equaform-engine::plugin`): convenience wrapper that
  combines `EngineHost` with `PluginAdapter<EngineHost>`.

- **`PluginState`** (in `equaform-common::plugin`): enum tracking lifecycle
  state (`Created`, `Ready`, `Processing`, `Deactivated`, `Destroyed`).

### Dependency graph

```
equaform-common (plugin module)
    ↑
    ├── equaform-engine (plugin module)
    │       ↑
    │       ├── equaform-cli
    │       ├── equaform-standalone
    │       └── equaform-integration-tests
    │
    └── equaform-plugin (re-exports equaform_common::plugin::*)
```

## Consequences

### Positive
- **No circular dependencies**: traits in `equaform-common`, implementations
  in `equaform-engine`, re-exports in `equaform-plugin`.
- **Host SDK isolation**: host-specific types (JUCE `AudioProcessor`, VST3
  `AudioEffect`, etc.) live only in `equaform-engine`, not in the plugin
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
- `crates/equaform-common/src/plugin.rs` — trait definitions
- `crates/equaform-engine/src/plugin.rs` — `EngineHost` and `EnginePlugin`
- `crates/equaform-plugin/src/lib.rs` — re-exports
- `tasks.json` — M2-T003
