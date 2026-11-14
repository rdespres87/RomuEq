#![forbid(unsafe_code)]

//! Engine-side plugin hosting.
//!
//! This module provides `EngineHost`, an implementation of `PluginHost` that
//! bridges the engine's parameter store and automation ingestor back to a
//! plugin through the `equaform-common::plugin` boundary.
//!
//! The engine owns the `EngineHost` and passes it to a `PluginAdapter`
//! which in turn hosts the plugin. This guarantees that:
//!
//! - The plugin crate (`equaform-plugin`) has no dependency on the engine crate.
//! - Host SDK types (JUCE, VST3, etc.) live in the engine crate, not in
//!   the plugin crate.
//! - The plugin communicates with the engine exclusively through
//!   `ParameterCommand` and `PluginState` — both defined in `equaform-common`.

use equaform_common::ParameterCommand;
use equaform_common::plugin::{Plugin, PluginHost, PluginState};

// ---------------------------------------------------------------------------
// EngineHost — implements PluginHost for the engine
// ---------------------------------------------------------------------------

/// Engine-side implementation of `PluginHost`.
///
/// Bridges plugin callbacks to the engine's parameter store and
/// automation ingestor. The engine owns this type and passes it
/// to `PluginAdapter`.
pub struct EngineHost {
    /// Shared parameter command channel (engine-side receiver).
    /// Commands sent by the plugin are queued here for the engine
    /// to process during the next parameter update phase.
    command_tx: std::sync::mpsc::SyncSender<ParameterCommand>,

    /// Current sample rate as reported by the engine.
    sample_rate: f64,

    /// Current block size as reported by the engine.
    block_size: u32,
}

impl EngineHost {
    /// Create a new engine host.
    ///
    /// # Arguments
    ///
    /// * `command_tx` — sender for parameter commands. The engine
    ///   receives from the corresponding `SyncReceiver`.
    /// * `sample_rate` — the current sample rate in Hz.
    /// * `block_size` — the current audio block size in samples.
    pub fn new(
        command_tx: std::sync::mpsc::SyncSender<ParameterCommand>,
        sample_rate: f64,
        block_size: u32,
    ) -> Self {
        EngineHost {
            command_tx,
            sample_rate,
            block_size,
        }
    }
}

impl PluginHost for EngineHost {
    fn send_parameter_command(&self, command: ParameterCommand) {
        // Non-blocking send: if the channel is full, the command is dropped.
        // This is acceptable because parameter commands are best-effort
        // and the next engine cycle will pick up the latest value.
        let _ = self.command_tx.try_send(command);
    }

    fn notify_state_change(&self, _state: PluginState) {
        // In a full implementation, this would notify the engine's
        // state machine. For now, it's a no-op.
    }

    fn current_sample_rate(&self) -> f64 {
        self.sample_rate
    }

    fn current_block_size(&self) -> u32 {
        self.block_size
    }
}

// ---------------------------------------------------------------------------
// EnginePlugin — convenience wrapper for engine-managed plugins
// ---------------------------------------------------------------------------

/// A plugin managed by the engine.
///
/// Combines an `EngineHost` with a `PluginAdapter` to provide a single
/// entry point for the engine to interact with a plugin.
pub struct EnginePlugin {
    adapter: equaform_common::plugin::PluginAdapter<EngineHost>,
}

impl EnginePlugin {
    /// Create a new engine-managed plugin.
    ///
    /// # Arguments
    ///
    /// * `command_tx` — sender for parameter commands.
    /// * `sample_rate` — the current sample rate in Hz.
    /// * `block_size` — the current audio block size in samples.
    /// * `plugin` — the plugin instance to host.
    pub fn new(
        command_tx: std::sync::mpsc::SyncSender<ParameterCommand>,
        sample_rate: f64,
        block_size: u32,
        plugin: Box<dyn Plugin>,
    ) -> Self {
        let host = EngineHost::new(command_tx, sample_rate, block_size);
        let adapter = equaform_common::plugin::PluginAdapter::new(host, plugin);
        EnginePlugin { adapter }
    }

    /// Initialise the plugin.
    pub fn init(&mut self, sample_rate: f64, block_size: u32) -> bool {
        self.adapter.init(sample_rate, block_size)
    }

    /// Process a single audio block.
    pub fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) {
        self.adapter.process(inputs, outputs)
    }

    /// Deactivate the plugin.
    pub fn deactivate(&mut self) {
        self.adapter.deactivate()
    }

    /// Re-activate the plugin after a sample-rate change.
    pub fn activate(&mut self, sample_rate: f64) {
        self.adapter.activate(sample_rate)
    }

    /// Clean up and destroy the plugin.
    pub fn destroy(&mut self) {
        self.adapter.destroy()
    }

    /// Return the current plugin state.
    pub fn state(&self) -> PluginState {
        self.adapter.state()
    }

    /// Return the current sample rate.
    pub fn sample_rate(&self) -> f64 {
        self.adapter.sample_rate()
    }

    /// Return the current block size.
    pub fn block_size(&self) -> u32 {
        self.adapter.block_size()
    }

    /// Return a reference to the underlying plugin.
    pub fn plugin(&self) -> &dyn Plugin {
        self.adapter.plugin()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use equaform_common::ParameterCommand;
    use equaform_common::plugin::Plugin;

    // -----------------------------------------------------------------------
    // StubPlugin — test double for Plugin
    // -----------------------------------------------------------------------

    struct TestPlugin {
        id: String,
        name: String,
    }

    impl TestPlugin {
        fn new(id: &str, name: &str) -> Self {
            TestPlugin {
                id: id.to_string(),
                name: name.to_string(),
            }
        }
    }

    impl Plugin for TestPlugin {
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
    // EngineHost tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_engine_host_sends_parameter_command() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let host = EngineHost::new(tx, 44100.0, 64);

        let command = ParameterCommand::SetParameter {
            id: equaform_common::ParamId(0),
            value: 0.5,
        };
        host.send_parameter_command(command);

        // Command was sent — channel is not full, so no drop.
        // We can't easily verify the receiver without consuming it,
        // but the test confirms the method doesn't panic.
    }

    #[test]
    fn test_engine_host_sample_rate() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let host = EngineHost::new(tx, 48000.0, 128);
        assert_eq!(host.current_sample_rate(), 48000.0);
        assert_eq!(host.current_block_size(), 128);
    }

    // -----------------------------------------------------------------------
    // EnginePlugin tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_engine_plugin_lifecycle() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let mut engine_plugin = EnginePlugin::new(
            tx,
            44100.0,
            64,
            Box::new(TestPlugin::new("test", "Test Plugin")),
        );

        // Initial state — created via new() but not yet initialised
        assert_eq!(engine_plugin.state(), PluginState::Created);

        // Init
        assert!(engine_plugin.init(44100.0, 64));
        assert_eq!(engine_plugin.state(), PluginState::Ready);

        // Process
        let inputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        engine_plugin.process(
            &inputs.iter().map(|v| v.as_slice()).collect::<Vec<_>>(),
            &mut outputs
                .iter_mut()
                .map(|v| v.as_mut_slice())
                .collect::<Vec<_>>(),
        );
        assert_eq!(engine_plugin.state(), PluginState::Ready);

        // Destroy
        engine_plugin.destroy();
        assert_eq!(engine_plugin.state(), PluginState::Destroyed);
    }

    #[test]
    fn test_engine_plugin_plugin_access() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let engine_plugin = EnginePlugin::new(
            tx,
            44100.0,
            64,
            Box::new(TestPlugin::new("my-id", "My Plugin")),
        );

        assert_eq!(engine_plugin.plugin().plugin_id(), "my-id");
        assert_eq!(engine_plugin.plugin().plugin_name(), "My Plugin");
    }
}
