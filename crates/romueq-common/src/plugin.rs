#![forbid(unsafe_code)]

//! Plugin boundary abstractions for RomuEq.
//!
//! This module defines the contract between the engine and host plugins.
//! It is placed in `romueq-common` so that both `romueq-engine` and
//! `romueq-plugin` can depend on it without creating a circular dependency.
//!
//! ## Design
//!
//! - **`PluginHost`** — trait that the host implements. The plugin calls
//!   these methods to request services (parameter commands, lifecycle
//!   notifications).
//! - **`Plugin`** — trait that a plugin implements. The engine calls
//!   these methods to interact with the plugin.
//! - **`PluginAdapter`** — bridge type that hosts a plugin instance and
//!   manages its lifecycle.
//!
//! This separation guarantees that the plugin crate has no dependency on
//! any host SDK (JUCE, VST3, AU, etc.).

use super::ParameterCommand;

// ---------------------------------------------------------------------------
// Plugin lifecycle states
// ---------------------------------------------------------------------------

/// Current state of a plugin instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginState {
    /// Plugin has been created but not yet initialised.
    Created,
    /// Plugin is initialised and ready to process.
    Ready,
    /// Plugin is currently processing audio.
    Processing,
    /// Plugin has been deactivated (e.g. sample-rate change).
    Deactivated,
    /// Plugin has been destroyed.
    Destroyed,
}

impl Default for PluginState {
    fn default() -> Self {
        Self::Created
    }
}

// ---------------------------------------------------------------------------
// Plugin trait — implemented by plugin authors
// ---------------------------------------------------------------------------

/// Interface that every plugin must implement.
///
/// The engine calls these methods in a defined lifecycle sequence.
/// Implementations must be RT-safe (no allocation, no blocking).
pub trait Plugin {
    /// Return the plugin's stable identifier.
    fn plugin_id(&self) -> &str;

    /// Return the plugin's human-readable name.
    fn plugin_name(&self) -> &str;

    /// Initialise the plugin. Called once after creation.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` — the target sample rate in Hz.
    /// * `block_size` — the initial audio block size in samples.
    ///
    /// # Returns
    ///
    /// `true` if initialisation succeeded, `false` otherwise.
    fn init(&mut self, sample_rate: f64, block_size: u32) -> bool;

    /// Process a single audio block.
    ///
    /// # Arguments
    ///
    /// * `inputs` — array of input buffers (one per channel).
    /// * `outputs` — array of output buffers (one per channel).
    ///
    /// Implementations must handle `inputs` and `outputs` potentially
    /// aliasing (same buffer for input and output).
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]);

    /// Deactivate the plugin (e.g. before a sample-rate change).
    fn deactivate(&mut self);

    /// Re-activate the plugin after a sample-rate change.
    fn activate(&mut self, sample_rate: f64);

    /// Clean up and prepare for destruction.
    fn cleanup(&mut self);
}

// ---------------------------------------------------------------------------
// PluginHost trait — implemented by the host/engine
// ---------------------------------------------------------------------------

/// Services that the host provides to the plugin.
///
/// The plugin calls these methods to request parameter changes,
/// automation events, and other host services.
pub trait PluginHost {
    /// Send a parameter command to the engine.
    ///
    /// This is the primary mechanism for the plugin to modify
    /// parameter values. Commands are queued and processed by
    /// the engine's parameter store.
    fn send_parameter_command(&self, command: ParameterCommand);

    /// Notify the host that the plugin's state has changed.
    fn notify_state_change(&self, state: PluginState);

    /// Request the current sample rate from the host.
    fn current_sample_rate(&self) -> f64;

    /// Request the current block size from the host.
    fn current_block_size(&self) -> u32;
}

// ---------------------------------------------------------------------------
// PluginAdapter — bridges engine and plugin
// ---------------------------------------------------------------------------

/// Hosts a plugin instance and manages its lifecycle.
///
/// The adapter owns the `PluginHost` implementation and delegates
/// plugin callbacks back through it. It is the engine's interface
/// to a plugin.
pub struct PluginAdapter<H: PluginHost> {
    host: H,
    plugin: Box<dyn Plugin>,
    state: PluginState,
    sample_rate: f64,
    block_size: u32,
}

impl<H: PluginHost> PluginAdapter<H> {
    /// Create a new plugin adapter.
    ///
    /// # Arguments
    ///
    /// * `host` — the host implementation providing services.
    /// * `plugin` — the plugin instance to host.
    pub fn new(host: H, plugin: Box<dyn Plugin>) -> Self {
        PluginAdapter {
            host,
            plugin,
            state: PluginState::Created,
            sample_rate: 44100.0,
            block_size: 64,
        }
    }

    /// Initialise the plugin.
    ///
    /// Returns `true` if initialisation succeeded.
    pub fn init(&mut self, sample_rate: f64, block_size: u32) -> bool {
        if self.state != PluginState::Created {
            return false;
        }

        let success = self.plugin.init(sample_rate, block_size);
        if success {
            self.sample_rate = sample_rate;
            self.block_size = block_size;
            self.state = PluginState::Ready;
            self.host.notify_state_change(PluginState::Ready);
        }
        success
    }

    /// Process a single audio block.
    pub fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) {
        if self.state != PluginState::Ready {
            return;
        }

        self.state = PluginState::Processing;
        self.host.notify_state_change(PluginState::Processing);

        self.plugin.process(inputs, outputs);

        self.state = PluginState::Ready;
        self.host.notify_state_change(PluginState::Ready);
    }

    /// Deactivate the plugin.
    pub fn deactivate(&mut self) {
        if self.state == PluginState::Processing || self.state == PluginState::Ready {
            self.plugin.deactivate();
            self.state = PluginState::Deactivated;
            self.host.notify_state_change(PluginState::Deactivated);
        }
    }

    /// Re-activate the plugin after a sample-rate change.
    pub fn activate(&mut self, sample_rate: f64) {
        if self.state == PluginState::Deactivated {
            self.plugin.activate(sample_rate);
            self.sample_rate = sample_rate;
            self.state = PluginState::Ready;
            self.host.notify_state_change(PluginState::Ready);
        }
    }

    /// Clean up and destroy the plugin.
    pub fn destroy(&mut self) {
        if self.state == PluginState::Destroyed {
            return;
        }

        self.plugin.cleanup();
        self.state = PluginState::Destroyed;
        self.host.notify_state_change(PluginState::Destroyed);
    }

    /// Return the current plugin state.
    pub fn state(&self) -> PluginState {
        self.state
    }

    /// Return the current sample rate.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Return the current block size.
    pub fn block_size(&self) -> u32 {
        self.block_size
    }

    /// Return a reference to the underlying plugin.
    pub fn plugin(&self) -> &dyn Plugin {
        self.plugin.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // StubPlugin — test double for Plugin
    // -----------------------------------------------------------------------

    struct StubPlugin {
        id: String,
        name: String,
    }

    impl StubPlugin {
        fn new(id: &str, name: &str) -> Self {
            StubPlugin {
                id: id.to_string(),
                name: name.to_string(),
            }
        }
    }

    impl Plugin for StubPlugin {
        fn plugin_id(&self) -> &str {
            &self.id
        }

        fn plugin_name(&self) -> &str {
            &self.name
        }

        fn init(&mut self, _sample_rate: f64, _block_size: u32) -> bool {
            true
        }

        fn process(&mut self, _inputs: &[&[f32]], _outputs: &mut [&mut [f32]]) {}

        fn deactivate(&mut self) {}

        fn activate(&mut self, _sample_rate: f64) {}

        fn cleanup(&mut self) {}
    }

    // -----------------------------------------------------------------------
    // StubHost — test double for PluginHost
    // -----------------------------------------------------------------------

    struct StubHost {
        last_command: std::cell::RefCell<Option<ParameterCommand>>,
        last_state: std::cell::RefCell<Option<PluginState>>,
        sample_rate: f64,
        block_size: u32,
    }

    impl StubHost {
        fn new(sample_rate: f64, block_size: u32) -> Self {
            StubHost {
                last_command: std::cell::RefCell::new(None),
                last_state: std::cell::RefCell::new(None),
                sample_rate,
                block_size,
            }
        }
    }

    impl PluginHost for StubHost {
        fn send_parameter_command(&self, command: ParameterCommand) {
            *self.last_command.borrow_mut() = Some(command);
        }

        fn notify_state_change(&self, state: PluginState) {
            *self.last_state.borrow_mut() = Some(state);
        }

        fn current_sample_rate(&self) -> f64 {
            self.sample_rate
        }

        fn current_block_size(&self) -> u32 {
            self.block_size
        }
    }

    // -----------------------------------------------------------------------
    // PluginAdapter tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_adapter_lifecycle() {
        let host = StubHost::new(44100.0, 64);
        let plugin = Box::new(StubPlugin::new("test", "Test Plugin"));
        let mut adapter = PluginAdapter::new(host, plugin);

        // Initial state
        assert_eq!(adapter.state(), PluginState::Created);

        // Init
        assert!(adapter.init(48000.0, 128));
        assert_eq!(adapter.state(), PluginState::Ready);
        assert_eq!(adapter.sample_rate(), 48000.0);
        assert_eq!(adapter.block_size(), 128);

        // Process
        let inputs: Vec<Vec<f32>> = vec![vec![0.0; 128]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 128]];
        adapter.process(
            &inputs.iter().map(|v| v.as_slice()).collect::<Vec<_>>(),
            &mut outputs
                .iter_mut()
                .map(|v| v.as_mut_slice())
                .collect::<Vec<_>>(),
        );
        assert_eq!(adapter.state(), PluginState::Ready);

        // Deactivate
        adapter.deactivate();
        assert_eq!(adapter.state(), PluginState::Deactivated);

        // Activate
        adapter.activate(44100.0);
        assert_eq!(adapter.state(), PluginState::Ready);

        // Destroy
        adapter.destroy();
        assert_eq!(adapter.state(), PluginState::Destroyed);
    }

    #[test]
    fn test_adapter_init_fails_if_not_created() {
        let host = StubHost::new(44100.0, 64);
        let plugin = Box::new(StubPlugin::new("test", "Test Plugin"));
        let mut adapter = PluginAdapter::new(host, plugin);

        // Init once — succeeds
        assert!(adapter.init(44100.0, 64));
        assert_eq!(adapter.state(), PluginState::Ready);

        // Init again — fails (not in Created state)
        assert!(!adapter.init(44100.0, 64));
        assert_eq!(adapter.state(), PluginState::Ready);
    }

    #[test]
    fn test_adapter_process_fails_if_not_ready() {
        let host = StubHost::new(44100.0, 64);
        let plugin = Box::new(StubPlugin::new("test", "Test Plugin"));
        let mut adapter = PluginAdapter::new(host, plugin);

        // Process before init — should be a no-op
        let inputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        adapter.process(
            &inputs.iter().map(|v| v.as_slice()).collect::<Vec<_>>(),
            &mut outputs
                .iter_mut()
                .map(|v| v.as_mut_slice())
                .collect::<Vec<_>>(),
        );
        assert_eq!(adapter.state(), PluginState::Created);
    }

    #[test]
    fn test_adapter_destroy_is_idempotent() {
        let host = StubHost::new(44100.0, 64);
        let plugin = Box::new(StubPlugin::new("test", "Test Plugin"));
        let mut adapter = PluginAdapter::new(host, plugin);

        adapter.init(44100.0, 64);
        adapter.destroy();
        assert_eq!(adapter.state(), PluginState::Destroyed);

        // Second destroy — no-op
        adapter.destroy();
        assert_eq!(adapter.state(), PluginState::Destroyed);
    }

    #[test]
    fn test_adapter_plugin_access() {
        let host = StubHost::new(44100.0, 64);
        let plugin = Box::new(StubPlugin::new("my-id", "My Plugin"));
        let adapter = PluginAdapter::new(host, plugin);

        assert_eq!(adapter.plugin().plugin_id(), "my-id");
        assert_eq!(adapter.plugin().plugin_name(), "My Plugin");
    }
}
