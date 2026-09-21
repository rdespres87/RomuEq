//! RomuEq CLAP Audio Plugin
//!
//! This crate provides a CLAP plugin implementation for the RomuEq audio engine.
//! It bridges the CLAP ABI via `clack-plugin` v0.1.x with the RomuEq engine lifecycle.

use clack_plugin::prelude::*;
use romueq_engine::lifecycle::EngineLifecycle;

// ---------------------------------------------------------------------------
// Plugin metadata / descriptor
// ---------------------------------------------------------------------------

/// Stable descriptor shared across all plugin instances.
fn get_descriptor() -> PluginDescriptor {
    let mut desc = PluginDescriptor::new("com.romueq.plugin", "RomuEq");
    desc = desc.with_vendor("RomuEq");
    desc = desc.with_version("0.1.0");
    desc = desc.with_description("Real-time spectral equalizer plugin");
    desc
}

// ---------------------------------------------------------------------------
// Thread-model types — unit structs implementing the clack-plugin traits
// ---------------------------------------------------------------------------

/// Shared (thread-safe) data — no per-instance state needed yet.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct RomuEqShared;

impl<'a> PluginShared<'a> for RomuEqShared {}

/// Main-thread data — no per-instance state needed yet.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct RomuEqMainThread;

impl<'a, S: PluginShared<'a>> PluginMainThread<'a, S> for RomuEqMainThread {}

// ---------------------------------------------------------------------------
// Audio processor — holds the engine and does DSP
// ---------------------------------------------------------------------------

/// The audio processor that bridges CLAP audio processing with the RomuEq engine.
pub struct RomuEqAudioProcessor {
    /// Internal engine managing equalization parameters and processing.
    engine: EngineLifecycle,
}

impl RomuEqAudioProcessor {
    /// Create a new audio processor with default configuration.
    fn new() -> Self {
        Self {
            engine: EngineLifecycle::new(),
        }
    }
}

impl<'a, S: PluginShared<'a>, M: PluginMainThread<'a, S>> PluginAudioProcessor<'a, S, M>
    for RomuEqAudioProcessor
{
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut M,
        _shared: &'a S,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        let mut processor = Self::new();

        // Determine channel count from the host-provided audio configuration.
        // We use a default of 2 (stereo) until the port enumeration API is wired up.
        let channels = 2;

        // Prepare the engine with host-provided sample rate and block size.
        processor.engine.prepare(
            audio_config.sample_rate,
            audio_config.max_frames_count as usize,
            channels,
        );

        Ok(processor)
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        _events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        // Collect all channel data into owned buffers so we can pass them
        // to the engine's slice-of-slices process() API.
        let mut all_input: Vec<Vec<f32>> = Vec::new();
        let mut all_output: Vec<Vec<f32>> = Vec::new();

        for mut port_pair in &mut audio {
            let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                continue;
            };

            for channel_pair in channel_pairs {
                match channel_pair {
                    ChannelPair::InPlace(buf) => {
                        all_input.push(buf.to_vec());
                        all_output.push(vec![0.0; buf.len()]);
                    }
                    ChannelPair::InputOutput(input, output) => {
                        all_input.push(input.to_vec());
                        all_output.push(output.to_vec());
                    }
                    ChannelPair::InputOnly(buf) => {
                        all_input.push(buf.to_vec());
                        all_output.push(vec![0.0; buf.len()]);
                    }
                    ChannelPair::OutputOnly(buf) => {
                        all_input.push(vec![0.0; buf.len()]);
                        all_output.push(buf.to_vec());
                    }
                }
            }
        }

        let num_channels = all_input.len();
        if num_channels == 0 {
            return Ok(ProcessStatus::Continue);
        }

        // Build slice-of-slices for the engine's process() API.
        let input_refs: Vec<&[f32]> = all_input.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            all_output.iter_mut().map(|v| v.as_mut_slice()).collect();

        // Run DSP processing through the engine.
        let success = self.engine.process(&input_refs, &mut output_refs);
        if !success {
            return Ok(ProcessStatus::Continue);
        }

        // Copy processed output back to CLAP buffers.
        let mut idx = 0;
        for mut port_pair in &mut audio {
            let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                continue;
            };

            for channel_pair in channel_pairs {
                let out_slice = &all_output[idx];
                match channel_pair {
                    ChannelPair::InPlace(buf) => buf.copy_from_slice(out_slice),
                    ChannelPair::InputOutput(_, output) => output.copy_from_slice(out_slice),
                    ChannelPair::InputOnly(_) => {} // discard
                    ChannelPair::OutputOnly(buf) => buf.copy_from_slice(out_slice),
                }
                idx += 1;
            }
        }

        Ok(ProcessStatus::Continue)
    }

    fn deactivate(mut self, _main_thread: &mut M) {
        self.engine.destroy();
    }

    fn start_processing(&mut self) -> Result<(), PluginError> {
        Ok(())
    }

    fn stop_processing(&mut self) {}
}

// ---------------------------------------------------------------------------
// RomuEqPlugin — the marker type tying together the thread-model types
// ---------------------------------------------------------------------------

/// Marker struct that defines the plugin's thread model and factory behavior.
#[derive(Default)]
pub struct RomuEqPlugin;

impl Plugin for RomuEqPlugin {
    type Shared<'a> = RomuEqShared;
    type MainThread<'a> = RomuEqMainThread;
    type AudioProcessor<'a> = RomuEqAudioProcessor;

    fn declare_extensions(
        _builder: &mut PluginExtensions<Self>,
        _shared: Option<&Self::Shared<'_>>,
    ) {
        // Register CLAP extensions here (e.g. parameter, state, audio ports).
    }
}

impl DefaultPluginFactory for RomuEqPlugin {
    fn get_descriptor() -> PluginDescriptor {
        get_descriptor()
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(RomuEqShared)
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        _shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(RomuEqMainThread)
    }
}

// ---------------------------------------------------------------------------
// Entry point — the CLAP ABI entry function
// ---------------------------------------------------------------------------

clack_export_entry!(SinglePluginEntry::<RomuEqPlugin>);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_creation() {
        let processor = RomuEqAudioProcessor::new();
        assert_eq!(processor.engine.sample_rate(), 44100.0);
    }

    #[test]
    fn test_plugin_default_shared() {
        let shared = RomuEqShared::default();
        assert_eq!(shared, RomuEqShared);
    }

    #[test]
    fn test_plugin_default_main_thread() {
        let mt = RomuEqMainThread::default();
        assert_eq!(mt, RomuEqMainThread);
    }

    #[test]
    fn test_plugin_descriptor() {
        let desc = get_descriptor();
        assert!(desc.id().is_some());
        assert!(desc.name().is_some());
    }

    #[test]
    fn test_plugin_marker_default() {
        let _plugin = RomuEqPlugin::default();
    }
}
