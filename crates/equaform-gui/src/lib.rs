#![deny(unsafe_code)]

//! GUI framework contract for Equaform.
//!
//! This crate defines the interface between the GUI layer and the engine layer.
//! The GUI must communicate with the engine via commands and queries — no direct
//! DSP state access.
//!
//! ## Framework Decision (ADR-003)
//!
//! **egui** (with winit backend for standalone mode) is the selected GUI framework.
//!
//! ## Lifecycle Contract
//!
//! The GUI follows a four-phase lifecycle aligned with the engine:
//!
//! 1. **create** — GUI context and window creation (not realtime-critical)
//! 2. **prepare** — engine prepared with sample rate, block size, channels
//! 3. **run** — render loop + command processing (non-blocking)
//! 4. **destroy** — clean up GUI resources, signal engine to destroy
//!
//! ## Thread Model
//!
//! - **GUI thread** (main): owns egui context, processes user input, renders UI
//! - **Audio thread**: owns engine, processes audio blocks (not directly visible to GUI)
//! - **Command queue**: GUI → engine (parameter commands, preset loads)
//! - **Query interface**: engine → GUI (current parameter values, analysis data)
//!
//! The command queue and query interface are the only cross-thread contracts.

// ---------------------------------------------------------------------------
// Command: GUI → engine direction
// ---------------------------------------------------------------------------

/// Unique parameter identifier (re-exports equaform_common for GUI convenience).
pub use equaform_common::parameter::ParamId;

use equaform_common::BandParameters;

#[cfg(feature = "standalone")]
pub mod audio;

/// A command from the GUI to the engine.
///
/// Each command targets exactly one parameter and carries the new value.
/// Commands are batched and processed by the engine on the next prepare/process cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiCommand {
    /// The parameter this command affects.
    pub param_id: ParamId,
    /// The new value to set (clamped by the GUI before sending).
    pub value: f32,
}

/// Batch of commands from the GUI, to be applied atomically.
#[derive(Debug, Clone, Default)]
pub struct GuiCommandBatch {
    commands: Vec<GuiCommand>,
}

impl GuiCommandBatch {
    /// Create a new empty batch.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Add a command to this batch.
    pub fn push(&mut self, param_id: ParamId, value: f32) {
        // De-duplicate: if the same param_id already exists, replace it.
        for cmd in &mut self.commands {
            if cmd.param_id == param_id {
                cmd.value = value;
                return;
            }
        }
        self.commands.push(GuiCommand { param_id, value });
    }

    /// Returns the commands in this batch.
    pub fn commands(&self) -> &[GuiCommand] {
        &self.commands
    }

    /// Returns true if this batch is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Consume this batch and return its commands.
    pub fn into_commands(self) -> Vec<GuiCommand> {
        self.commands
    }
}

// ---------------------------------------------------------------------------
// Query: engine → GUI direction
// ---------------------------------------------------------------------------

/// Current parameter value as reported by the engine.
#[derive(Debug, Clone)]
pub struct ParamValue {
    /// The parameter ID.
    pub param_id: ParamId,
    /// Current value set by the engine (after any automation/automation effects).
    pub value: f32,
}

/// Snapshot of all parameter values, as reported by the engine.
///
/// The GUI uses this to keep its widgets in sync with the actual DSP state.
#[derive(Debug, Clone)]
pub struct ParamSnapshot {
    values: Vec<ParamValue>,
}

impl ParamSnapshot {
    /// Create a new snapshot from the engine's parameter values.
    pub fn new(values: Vec<ParamValue>) -> Self {
        Self { values }
    }

    /// Returns all parameter values in the snapshot.
    pub fn values(&self) -> &[ParamValue] {
        &self.values
    }

    /// Get the value for a specific parameter, if present.
    pub fn get(&self, param_id: ParamId) -> Option<f32> {
        self.values
            .iter()
            .find(|v| v.param_id == param_id)
            .map(|v| v.value)
    }

    /// Set the value for a specific parameter.
    pub fn set(&mut self, param_id: ParamId, value: f32) {
        match self.values.iter_mut().find(|v| v.param_id == param_id) {
            Some(v) => v.value = value,
            None => self.values.push(ParamValue { param_id, value }),
        }
    }

    /// Returns true if this snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the number of parameters in this snapshot.
    pub fn len(&self) -> usize {
        self.values.len()
    }
}

/// Current analysis data as reported by the engine's analysis pipeline.
#[derive(Debug, Clone)]
pub struct AnalysisSnapshot {
    /// RMS values per channel (dBFS, or -inf for silence).
    pub rms: [f32; equaform_common::ANALYSIS_MAX_CHANNELS],
    /// Peak values per channel (dBFS, or -inf for silence).
    pub peak: [f32; equaform_common::ANALYSIS_MAX_CHANNELS],
    /// Whether the last frame had dropped data.
    pub dropped: bool,
    /// Log-scaled spectrum data: Vec of [center_freq_hz, magnitude_db] pairs.
    /// Empty (zero length) when no spectrum data is available yet.
    pub spectrum_data: Vec<[f64; 2]>,
}

impl Default for AnalysisSnapshot {
    fn default() -> Self {
        Self {
            rms: [f32::NEG_INFINITY; equaform_common::ANALYSIS_MAX_CHANNELS],
            peak: [f32::NEG_INFINITY; equaform_common::ANALYSIS_MAX_CHANNELS],
            dropped: false,
            spectrum_data: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Band parameter bindings (stable ParamId mappings)
// ---------------------------------------------------------------------------

/// Offset from a band base ID to each parameter's stable ID.
pub use equaform_common::parameter::band;

/// All parameters for a single EQ band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandParams {
    /// Band index (0–7).
    pub band_index: usize,
    /// Base parameter ID for this band (e.g. 100 for band 0).
    pub base_id: u32,
}

impl BandParams {
    /// Create a new band parameter set.
    pub const fn new(band_index: usize, base_id: u32) -> Self {
        Self {
            band_index,
            base_id,
        }
    }

    /// Return the ParamId for a specific band parameter.
    pub fn param_id(&self, offset: u32) -> ParamId {
        ParamId(self.base_id + offset)
    }

    /// Returns the enabled parameter ID.
    pub fn enabled(&self) -> ParamId {
        self.param_id(band::ENABLED)
    }

    /// Returns the filter type parameter ID.
    pub fn filter_type(&self) -> ParamId {
        self.param_id(band::FILTER_TYPE)
    }

    /// Returns the frequency HZ parameter ID.
    pub fn frequency_hz(&self) -> ParamId {
        self.param_id(band::FREQUENCY_HZ)
    }

    /// Returns the gain dB parameter ID.
    pub fn gain_db(&self) -> ParamId {
        self.param_id(band::GAIN_DB)
    }

    /// Returns the Q parameter ID.
    pub fn q(&self) -> ParamId {
        self.param_id(band::Q)
    }

    /// Returns the slope parameter ID.
    pub fn slope(&self) -> ParamId {
        self.param_id(band::SLOPE)
    }

    /// Returns the human-readable band label.
    pub fn label(&self) -> String {
        format!("Band {}", self.band_index + 1)
    }
}

// ---------------------------------------------------------------------------
// Band editor widget state
// ---------------------------------------------------------------------------

/// Per-band editing state for egui widgets.
///
/// Tracks which parameters are currently being edited (keyboard focus).
#[derive(Clone, Debug)]
pub struct BandEditor {
    /// Currently editing parameter ID (None when not focused).
    pub editing: Option<ParamId>,
}

impl BandEditor {
    /// Create a new empty editor.
    pub fn new() -> Self {
        Self { editing: None }
    }

    /// Returns true if the given parameter is currently being edited.
    pub fn is_editing(&self, param_id: ParamId) -> bool {
        self.editing == Some(param_id)
    }

    /// Set the currently editing parameter.
    pub fn start_editing(&mut self, param_id: ParamId) {
        self.editing = Some(param_id);
    }

    /// Clear the editing state.
    pub fn stop_editing(&mut self) {
        self.editing = None;
    }
}

impl Default for BandEditor {
    fn default() -> Self {
        Self::new()
    }
}

/// Editor state for all 8 bands.
#[derive(Clone, Debug)]
pub struct BandControls {
    editors: [BandEditor; equaform_common::MAX_BANDS],
}

impl BandControls {
    /// Create a new empty band controls.
    pub fn new() -> Self {
        Self {
            editors: core::array::from_fn(|_| BandEditor::new()),
        }
    }

    /// Get the editor for a specific band.
    pub fn editor(&mut self, band_index: usize) -> &mut BandEditor {
        &mut self.editors[band_index]
    }

    /// Returns the band parameters for a given index.
    pub fn params(&self, band_index: usize) -> BandParams {
        BandParams::new(band_index, (band_index as u32 + 1) * 100)
    }
}

impl Default for BandControls {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the 8 default parametric EQ bands aligned across standard musical zones
/// [32, 75, 180, 420, 1000, 2400, 5600, 13500] Hz.
pub fn default_8_bands() -> [equaform_common::BandParameters; equaform_common::MAX_BANDS] {
    let freqs = [32.0, 75.0, 180.0, 420.0, 1000.0, 2400.0, 5600.0, 13500.0];
    let mut bands = [equaform_common::BandParameters::default(); equaform_common::MAX_BANDS];
    for (i, b) in bands.iter_mut().enumerate() {
        b.enabled = true;
        b.frequency_hz = freqs[i];
        b.gain_db = 0.0;
        b.q = if i == 0 || i == 7 { 0.707 } else { 1.0 };
        b.filter_type = if i == 0 {
            equaform_common::FilterType::LowShelf
        } else if i == 7 {
            equaform_common::FilterType::HighShelf
        } else {
            equaform_common::FilterType::Bell
        };
    }
    bands
}


// ---------------------------------------------------------------------------
// Engine interface contract (implemented by equaform-engine)
// ---------------------------------------------------------------------------

/// The interface the GUI uses to interact with the engine.
///
/// This trait is implemented by equaform-engine and provides a clean boundary:
/// the GUI knows nothing about DSP internals, only commands and queries.
///
/// ## Thread Safety
///
/// This trait is `!Sync` by default — the GUI owns a mutable reference to
/// the engine during the run phase. For multi-threaded scenarios, the engine
/// can be wrapped in a mutex or channel-based message queue.
pub trait GuiClient {
    /// Prepare the engine for audio processing.
    ///
    /// Called once after GUI creation and whenever sample rate, block size,
    /// or channel count changes.
    fn prepare(&mut self, sample_rate: f32, block_size: usize, channels: usize);

    /// Process a single audio block.
    ///
    /// The engine applies any pending GUI commands before processing the block.
    fn process(&mut self, input: &[f32], output: &mut [f32]);

    /// Get the current parameter snapshot from the engine.
    ///
    /// The GUI calls this once per frame to keep widgets in sync.
    fn snapshot(&self) -> ParamSnapshot;

    /// Get the current analysis snapshot from the engine's analysis pipeline.
    fn analysis_snapshot(&self) -> AnalysisSnapshot;

    /// Apply a batch of commands from the GUI.
    ///
    /// Commands are applied immediately (not deferred) to keep the GUI responsive.
    fn apply_commands(&mut self, batch: GuiCommandBatch);

    /// Destroy the engine and release all resources.
    fn destroy(&mut self);

    /// Returns true if the engine is currently prepared (after prepare(), before destroy()).
    fn is_prepared(&self) -> bool;

    /// Create a preset from the current state and add it to the manager.
    fn create_preset(&mut self, name: &str) -> Result<String, String>;

    /// Apply a preset by name to the current state.
    fn apply_preset(&mut self, name: &str) -> Result<(), String>;

    /// List all stored presets.
    fn list_presets(&self) -> Vec<String>;

    /// Remove a preset by name.
    fn remove_preset(&mut self, name: &str) -> bool;

    /// Save a preset to a file on disk.
    fn save_preset_to_file(&mut self, name: &str, path: &str) -> Result<(), String>;

    /// Load a preset from a file on disk.
    fn load_preset_from_file(&mut self, path: &str) -> Result<String, String>;

    /// Returns the egui context for rendering.
    fn egui_context(&self) -> &egui::Context;

    /// Generates the next auto-incrementing preset name.
    fn next_auto_name(&self) -> String;

    /// Returns the current band parameters for a given band index.
    fn band_parameters(&self, band_index: usize) -> Option<BandParameters>;

    /// Returns a reference to the current spectrum data (FFT bins).
    fn spectrum_data(&self) -> &[[f64; 2]];
}

// ---------------------------------------------------------------------------
// GUI Application trait (implemented by concrete egui app)
// ---------------------------------------------------------------------------

/// The GUI application interface.
///
/// Implement this trait to create a concrete egui application.
/// The framework handles window creation, rendering, and event processing.
pub trait GuiApp: Send {
    /// Create a new GUI app instance.
    fn new() -> Self;

    /// Called every frame to update and render the UI.
    ///
    /// The implementation should:
    /// 1. Process any new commands from user input
    /// 2. Query the engine for current state
    /// 3. Render the UI using egui widgets
    fn update(&mut self, client: &mut dyn GuiClient);

    /// Returns the window title for this GUI application.
    fn window_title(&self) -> &str {
        "Equaform-8"
    }

    /// Returns the window size hint (width, height) in physical pixels.
    fn window_size(&self) -> (f32, f32) {
        (800.0, 600.0)
    }
}

// ---------------------------------------------------------------------------
// Preset manager module
// ---------------------------------------------------------------------------

pub mod preset_manager;
pub use preset_manager::{NamedPreset, PresetManager, PresetManagerError};

pub mod presets_io;
pub mod workspace_ui;
pub use presets_io::*;
pub use workspace_ui::*;

// ---------------------------------------------------------------------------
// Standalone entry point (feature-gated)
// ---------------------------------------------------------------------------


pub mod widgets;
pub use widgets::{
    render_band, render_frequency_response_plot, render_interactive_frequency_response_plot,
    render_master_section, render_spectrum_analyzer, render_stereo_vu_meter,
    render_vertical_stereo_vu_meter,
};

#[cfg(feature = "standalone")]
pub mod standalone;


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_band_params_id_mapping() {
        let band0 = BandParams::new(0, 100);
        assert_eq!(band0.enabled(), ParamId(100));
        assert_eq!(band0.filter_type(), ParamId(101));
        assert_eq!(band0.frequency_hz(), ParamId(102));
        assert_eq!(band0.gain_db(), ParamId(103));
        assert_eq!(band0.q(), ParamId(104));
        assert_eq!(band0.slope(), ParamId(105));

        let band7 = BandParams::new(7, 800);
        assert_eq!(band7.enabled(), ParamId(800));
        assert_eq!(band7.gain_db(), ParamId(803));
    }

    #[test]
    fn test_band_editor_state() {
        let mut editor = BandEditor::new();
        assert!(!editor.is_editing(ParamId(100)));

        editor.start_editing(ParamId(100));
        assert!(editor.is_editing(ParamId(100)));
        assert!(!editor.is_editing(ParamId(101)));

        editor.stop_editing();
        assert!(!editor.is_editing(ParamId(100)));
    }

    #[test]
    fn test_band_controls_new() {
        let controls = BandControls::new();
        assert_eq!(controls.params(0).band_index, 0);
        assert_eq!(controls.params(7).band_index, 7);
    }

    #[test]
    fn test_gui_command_batch_dedup() {
        let mut batch = GuiCommandBatch::new();
        batch.push(ParamId(100), 1.0);
        batch.push(ParamId(100), 2.0); // should replace
        assert_eq!(batch.commands().len(), 1);
        assert_eq!(batch.commands()[0].value, 2.0);
    }

    #[test]
    fn test_gui_command_batch_empty() {
        let batch = GuiCommandBatch::new();
        assert!(batch.is_empty());
    }

    #[test]
    fn test_param_snapshot_get() {
        let snap = ParamSnapshot::new(vec![
            ParamValue {
                param_id: ParamId(100),
                value: 1.0,
            },
            ParamValue {
                param_id: ParamId(103),
                value: 6.5,
            },
        ]);

        assert_eq!(snap.get(ParamId(100)), Some(1.0));
        assert_eq!(snap.get(ParamId(103)), Some(6.5));
        assert_eq!(snap.get(ParamId(101)), None);
    }

    #[test]
    fn test_analysis_snapshot_default() {
        let snap = AnalysisSnapshot::default();
        assert!(!snap.dropped);
        assert_eq!(snap.rms[0], f32::NEG_INFINITY);
        assert_eq!(snap.peak[0], f32::NEG_INFINITY);
    }

    #[test]
    fn test_band_label() {
        assert_eq!(BandParams::new(0, 100).label(), "Band 1");
        assert_eq!(BandParams::new(7, 800).label(), "Band 8");
    }

    #[test]
    fn test_gui_command_batch_multiple_params() {
        let mut batch = GuiCommandBatch::new();
        batch.push(ParamId(100), 1.0);
        batch.push(ParamId(101), 2.0);
        batch.push(ParamId(102), 3.0);

        assert_eq!(batch.commands().len(), 3);
    }

    #[test]
    fn test_gui_command_batch_order_preserved() {
        let mut batch = GuiCommandBatch::new();
        batch.push(ParamId(105), 1.0);
        batch.push(ParamId(102), 2.0);
        batch.push(ParamId(103), 3.0);

        // Commands should be in insertion order
        assert_eq!(batch.commands()[0].param_id, ParamId(105));
        assert_eq!(batch.commands()[1].param_id, ParamId(102));
        assert_eq!(batch.commands()[2].param_id, ParamId(103));
    }

    #[test]
    fn test_param_snapshot_new() {
        let snap = ParamSnapshot::new(vec![ParamValue {
            param_id: ParamId(100),
            value: 1.5,
        }]);
        assert_eq!(snap.get(ParamId(100)), Some(1.5));
        assert_eq!(snap.get(ParamId(999)), None);
    }

    #[test]
    fn test_band_params_default_values() {
        let bp = BandParams::new(0, 100);
        assert_eq!(bp.band_index, 0);
        // frequency_hz() returns ParamId(base_id + band::FREQUENCY_HZ) = ParamId(102)
        assert_eq!(bp.frequency_hz(), ParamId(102));
        assert_eq!(bp.enabled(), ParamId(100));
        assert_eq!(bp.gain_db(), ParamId(103));
    }

    #[test]
    fn test_band_params_gain_range() {
        let bp = BandParams::new(0, 100);
        // ParamId for gain should be in the valid range
        let gain_id = bp.gain_db();
        assert!(gain_id.0 >= 1 && gain_id.0 <= 999);
    }

    #[test]
    fn test_band_params_slope_range() {
        let bp = BandParams::new(0, 100);
        let slope_id = bp.slope();
        assert!(slope_id.0 >= 1 && slope_id.0 <= 999);
    }

    #[test]
    fn test_band_editor_start_and_stop_editing() {
        let mut editor = BandEditor::new();

        // Start editing a band parameter
        editor.start_editing(ParamId(100));
        assert!(editor.is_editing(ParamId(100)));
        assert!(!editor.is_editing(ParamId(101)));

        // Stop editing
        editor.stop_editing();
        assert!(!editor.is_editing(ParamId(100)));

        // Start editing another parameter
        editor.start_editing(ParamId(200));
        assert!(editor.is_editing(ParamId(200)));
        assert!(!editor.is_editing(ParamId(100)));
    }

    #[test]
    fn test_band_editor_multiple_bands() {
        let mut editor = BandEditor::new();

        // start_editing replaces the previous editing param (single-param model)
        editor.start_editing(ParamId(100));
        assert!(editor.is_editing(ParamId(100)));

        // Next start_editing replaces the previous
        editor.start_editing(ParamId(200));
        assert!(!editor.is_editing(ParamId(100)));
        assert!(editor.is_editing(ParamId(200)));

        editor.stop_editing();
        assert!(!editor.is_editing(ParamId(100)));
        assert!(!editor.is_editing(ParamId(200)));
    }

    #[test]
    fn test_band_controls_params() {
        let controls = BandControls::new();

        // Verify each band has valid parameters
        for i in 0..equaform_common::MAX_BANDS {
            let bp = controls.params(i);
            assert_eq!(bp.band_index, i);
            // Frequency param ID should be in a valid range
            let freq_id = bp.frequency_hz();
            assert!(freq_id.0 > 0);
        }
    }

    #[test]
    fn test_gui_command_batch_push_same_id_replaces() {
        let mut batch = GuiCommandBatch::new();

        batch.push(ParamId(100), 1.0);
        assert_eq!(batch.commands().len(), 1);
        assert_eq!(batch.commands()[0].value, 1.0);

        batch.push(ParamId(100), 5.0);
        assert_eq!(batch.commands().len(), 1);
        assert_eq!(batch.commands()[0].value, 5.0);
    }

    #[test]
    fn test_analysis_snapshot_rms_and_peak() {
        let snap = AnalysisSnapshot::default();

        // Default should have negative infinity for RMS and peak
        let channels = equaform_common::ANALYSIS_MAX_CHANNELS;
        assert_eq!(snap.rms.len(), channels);
        assert_eq!(snap.peak.len(), channels);
        for i in 0..channels {
            assert_eq!(snap.rms[i], f32::NEG_INFINITY);
            assert_eq!(snap.peak[i], f32::NEG_INFINITY);
        }
    }

    #[test]
    fn test_analysis_snapshot_dropped_flag() {
        let snap = AnalysisSnapshot::default();
        assert!(!snap.dropped);

        // Verify the dropped flag can be set
        let snap = AnalysisSnapshot {
            dropped: true,
            ..Default::default()
        };
        assert!(snap.dropped);
    }

    #[test]
    fn test_render_master_section_signature() {
        // Verify the function exists and has the correct signature
        let _fn: fn(&mut egui::Ui, &ParamSnapshot, &mut GuiCommandBatch) = render_master_section;
    }

    #[test]
    fn test_render_band_signature() {
        let _fn: fn(&mut egui::Ui, usize, &BandParams, &ParamSnapshot, &mut GuiCommandBatch) =
            render_band;
    }

    #[test]
    fn test_render_frequency_response_plot_signature() {
        // Verify the function exists and compiles with correct signature.
        // We can't easily create an egui::Ui in a unit test, so we just
        // check the type signature by assigning to a typed variable.
        let _fn: fn(&mut egui::Ui, &[equaform_common::BandParameters; 8], f32, usize) =
            render_frequency_response_plot;
        let _ = _fn;
    }

    #[test]
    fn test_render_spectrum_analyzer_signature() {
        let _fn: fn(&mut egui::Ui, &AnalysisSnapshot, f32, usize) = render_spectrum_analyzer;
    }

    #[test]
    fn test_band_params_filter_type() {
        let bp = BandParams::new(0, 100);
        let filter_type_id = bp.filter_type();
        // Filter type param ID should be in a valid range
        assert!(filter_type_id.0 >= 1 && filter_type_id.0 <= 999);
    }

    #[test]
    fn test_band_params_q() {
        let bp = BandParams::new(0, 100);
        let q_id = bp.q();
        // Q param ID should be in a valid range
        assert!(q_id.0 >= 1 && q_id.0 <= 999);
    }

    #[test]
    fn test_band_params_enabled_param() {
        let bp = BandParams::new(0, 100);
        let enabled_id = bp.enabled();
        assert!(enabled_id.0 >= 1 && enabled_id.0 <= 999);
    }

    #[test]
    fn test_band_params_frequency_hz_positive() {
        for i in 0..equaform_common::MAX_BANDS {
            let bp = BandControls::new().params(i);
            // frequency_hz() returns ParamId - verify it's a valid positive ID
            assert!(
                bp.frequency_hz().0 > 0,
                "Band {i} frequency param ID should be positive"
            );
        }
    }

    #[test]
    fn test_band_controls_max_bands() {
        let controls = BandControls::new();
        assert_eq!(controls.params(0).band_index, 0);
        assert_eq!(controls.params(7).band_index, 7);
        // Verify we can access all MAX_BANDS bands
        for i in 0..equaform_common::MAX_BANDS {
            let bp = controls.params(i);
            assert_eq!(bp.band_index, i);
        }
    }

    #[test]
    fn test_gui_command_batch_new_is_empty() {
        let batch = GuiCommandBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.commands().len(), 0);
    }
}
