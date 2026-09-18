#![forbid(unsafe_code)]

//! GUI framework contract for RomuEq.
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

/// Unique parameter identifier (re-exports romueq_common for GUI convenience).
pub use romueq_common::parameter::ParamId;

/// egui Widget trait for `.ui()` method on sliders, checkboxes, etc.
use egui::Widget;
use egui_plot::{Line, Plot};
use romueq_common::BandParameters;

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
    pub rms: [f32; romueq_common::ANALYSIS_MAX_CHANNELS],
    /// Peak values per channel (dBFS, or -inf for silence).
    pub peak: [f32; romueq_common::ANALYSIS_MAX_CHANNELS],
    /// Whether the last frame had dropped data.
    pub dropped: bool,
    /// Log-scaled spectrum data: Vec of [center_freq_hz, magnitude_db] pairs.
    /// Empty (zero length) when no spectrum data is available yet.
    pub spectrum_data: Vec<[f64; 2]>,
}

impl Default for AnalysisSnapshot {
    fn default() -> Self {
        Self {
            rms: [f32::NEG_INFINITY; romueq_common::ANALYSIS_MAX_CHANNELS],
            peak: [f32::NEG_INFINITY; romueq_common::ANALYSIS_MAX_CHANNELS],
            dropped: false,
            spectrum_data: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Band parameter bindings (stable ParamId mappings)
// ---------------------------------------------------------------------------

/// Offset from a band base ID to each parameter's stable ID.
pub use romueq_common::parameter::band;

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
    editors: [BandEditor; romueq_common::MAX_BANDS],
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

// ---------------------------------------------------------------------------
// Band widget rendering (egui integration)
// ---------------------------------------------------------------------------

/// Render a single band's controls.
///
/// Each control maps to exactly one stable ParamId and validates/clamps values
/// before emitting commands through the batch.
pub fn render_band(
    ui: &mut egui::Ui,
    band_index: usize,
    params: &BandParams,
    snapshot: &ParamSnapshot,
    batch: &mut GuiCommandBatch,
) {
    // Band header with enable checkbox.
    ui.horizontal(|ui| {
        let enabled_val = snapshot.get(params.enabled()).unwrap_or(1.0);
        let mut enabled = enabled_val >= 0.5;

        egui::Checkbox::new(&mut enabled, "")
            .ui(ui)
            .on_hover_text("Enable/disable band");

        if enabled {
            batch.push(params.enabled(), 0.0);
        } else {
            batch.push(params.enabled(), 1.0);
        }

        ui.label(params.label());

        // Enable/disable button.
        if ui
            .button(if enabled { "Disable" } else { "Enable" })
            .clicked()
        {
            let new_val = if enabled { 0.0 } else { 1.0 };
            batch.push(params.enabled(), new_val);
        }

        ui.separator();
    });

    // Filter type dropdown (enum, 0–4).
    let mut filter_type_val = snapshot.get(params.filter_type()).unwrap_or(0.0);
    ui.horizontal(|ui| {
        let filter_types = ["Lowpass", "Highpass", "Peaking", "Lowshelf", "Highshelf"];
        let idx = filter_type_val as usize;
        let selected = if idx < filter_types.len() {
            filter_types[idx]
        } else {
            "?"
        };

        egui::ComboBox::from_id_salt(format!("filter_type_{band_index}"))
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (i, name) in filter_types.iter().enumerate() {
                    if ui
                        .selectable_label(filter_type_val == i as f32, *name)
                        .clicked()
                    {
                        filter_type_val = i as f32;
                        batch.push(params.filter_type(), filter_type_val);
                    }
                }
            });
    });

    // Frequency slider (logarithmic scale, 20–20000 Hz).
    let mut freq_val: f32 = snapshot.get(params.frequency_hz()).unwrap_or(1000.0);
    ui.horizontal(|ui| {
        let freq_min: f32 = 20.0;
        let freq_max: f32 = 20_000.0;

        // Use logarithmic scale for better UX.
        let log_min = freq_val.log10().clamp(freq_min.log10(), freq_max.log10());
        let mut display_log = log_min;

        egui::Slider::new(&mut display_log, freq_min.log10()..=freq_max.log10())
            .suffix(" Hz")
            .ui(ui);

        // Update value on change.
        let new_val = 10.0_f32.powf(display_log);
        if new_val != freq_val {
            freq_val = new_val;
            batch.push(params.frequency_hz(), freq_val);
        }
    });

    // Gain slider (-24 to +24 dB).
    let mut gain_val: f32 = snapshot.get(params.gain_db()).unwrap_or(0.0);
    ui.horizontal(|ui| {
        egui::Slider::new(&mut gain_val, -24.0..=24.0)
            .suffix(" dB")
            .ui(ui);
        batch.push(params.gain_db(), gain_val);
    });

    // Q slider (0.1 to 20).
    let mut q_val: f32 = snapshot.get(params.q()).unwrap_or(0.707);
    ui.horizontal(|ui| {
        egui::Slider::new(&mut q_val, 0.1..=20.0)
            .suffix(" Q")
            .ui(ui);
        batch.push(params.q(), q_val);
    });

    // Slope dropdown (enum, 0–3: 6/12/24/48 dB/oct).
    let mut slope_val = snapshot.get(params.slope()).unwrap_or(1.0);
    ui.horizontal(|ui| {
        let slopes = ["6", "12", "24", "48"];
        let idx = slope_val as usize;
        let selected = if idx < slopes.len() { slopes[idx] } else { "?" };

        egui::ComboBox::from_id_salt(format!("slope_{band_index}"))
            .selected_text(format!("{selected} dB/oct"))
            .show_ui(ui, |ui| {
                for (i, label) in slopes.iter().enumerate() {
                    if ui
                        .selectable_label(slope_val == i as f32, format!("{label} dB/oct"))
                        .clicked()
                    {
                        slope_val = i as f32;
                        batch.push(params.slope(), slope_val);
                    }
                }
            });
    });

    ui.separator();
}

// ---------------------------------------------------------------------------
// Frequency response plot widget
// ---------------------------------------------------------------------------

/// Renders the EQ frequency response curve from band parameters.
///
/// Uses a logarithmic frequency axis (20 Hz – Nyquist) with dB magnitude
/// on the Y axis. Only enabled bands contribute to the response.
pub fn render_frequency_response_plot(
    ui: &mut egui::Ui,
    bands: &[romueq_common::BandParameters; romueq_common::MAX_BANDS],
    sample_rate: f32,
    width: usize,
) {
    use romueq_dsp::compute_response;

    let response = compute_response(sample_rate, bands, width);

    // Convert to plot points: [frequency_hz, magnitude_db]
    let points: Vec<[f64; 2]> = response
        .iter()
        .map(|p| [p.frequency_hz as f64, p.magnitude_db as f64])
        .collect();

    let line = Line::new(points)
        .color(egui::Color32::from_rgb(0, 180, 255))
        .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 180, 255)))
        .name("EQ Response");

    // Clamp to a reasonable dB range for display
    let plot = Plot::new("eq_response")
        .include_x(20.0)
        .include_x(24000.0)
        .include_y(-60.0)
        .include_y(30.0)
        .height(200.0)
        .show(ui, |plot_ui| plot_ui.line(line));

    // Show axis labels
    ui.horizontal(|ui| {
        ui.label("Frequency (Hz)");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label("Magnitude (dB)");
        });
    });

    let _ = plot;
}

// ---------------------------------------------------------------------------
// Spectrum analyzer widget
// ---------------------------------------------------------------------------

/// Renders a real-time spectrum analyzer from analysis data.
///
/// Displays the latest FFT magnitude data as a log-scaled bar chart.
/// Uses `analysis.spectrum_data` (a Vec of [freq_hz, magnitude_db] pairs)
/// when available; falls back to a flat line at -inf when no data exists.
pub fn render_spectrum_analyzer(
    ui: &mut egui::Ui,
    analysis: &AnalysisSnapshot,
    sample_rate: f32,
    num_bars: usize,
) {
    let nyquist = sample_rate * 0.5;

    // Build frequency bins (logarithmically spaced) for x-axis mapping.
    let log_min = 20.0_f64.log10();
    let log_max = (nyquist as f64).log10();

    let mut bars: Vec<[f64; 2]> = Vec::with_capacity(num_bars);

    if analysis.spectrum_data.is_empty() {
        // No spectrum data yet — draw a flat line at -inf.
        for i in 0..num_bars {
            let t = if num_bars == 1 {
                0.5_f64
            } else {
                i as f64 / (num_bars - 1) as f64
            };
            let center_freq = 10.0_f64.powf(log_min + t * (log_max - log_min));
            bars.push([center_freq, f64::NEG_INFINITY]);
        }
    } else {
        // Interpolate the FFT spectrum data onto logarithmic bins.
        let spectrum = &analysis.spectrum_data;

        for i in 0..num_bars {
            let t = if num_bars == 1 {
                0.5_f64
            } else {
                i as f64 / (num_bars - 1) as f64
            };
            let target_freq = 10.0_f64.powf(log_min + t * (log_max - log_min));

            // Find the two nearest spectrum data points for interpolation.
            let mut lower_idx = 0usize;
            let mut upper_idx = spectrum.len().saturating_sub(1);

            // Binary search for the insertion point.
            if spectrum.len() > 1 {
                let mut lo = 0usize;
                let mut hi = spectrum.len() - 1;
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    if spectrum[mid][0] < target_freq {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                if lo == 0 {
                    lower_idx = 0;
                    upper_idx = std::cmp::min(1, spectrum.len() - 1);
                } else if lo >= spectrum.len() {
                    lower_idx = spectrum.len() - 1;
                    upper_idx = spectrum.len() - 1;
                } else {
                    lower_idx = lo.saturating_sub(1);
                    upper_idx = lo;
                }
            }

            let mag_db = if spectrum[lower_idx][0] == spectrum[upper_idx][0] {
                spectrum[lower_idx][1]
            } else {
                // Linear interpolation between the two nearest points.
                let t_interp = (target_freq - spectrum[lower_idx][0])
                    / (spectrum[upper_idx][0] - spectrum[lower_idx][0]);
                let lo_val = spectrum[lower_idx][1];
                let hi_val = spectrum[upper_idx][1];
                lo_val + t_interp * (hi_val - lo_val)
            };

            bars.push([target_freq, mag_db]);
        }
    }

    let bar_chart = Line::new(bars)
        .color(egui::Color32::from_rgb(0, 255, 100))
        .name("Spectrum");

    let plot = Plot::new("spectrum")
        .include_x(20.0)
        .include_x(nyquist as f64)
        .include_y(-90.0)
        .include_y(0.0)
        .height(150.0)
        .show(ui, |plot_ui| plot_ui.line(bar_chart));

    // Axis labels
    ui.horizontal(|ui| {
        ui.label("Frequency (Hz)");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label("Level (dBFS)");
        });
    });

    let _ = plot;
}

// ---------------------------------------------------------------------------
// Master section controls widget
// ---------------------------------------------------------------------------

/// Renders the master section controls: bypass, output gain, and mute.
///
/// These are global parameters that affect the entire EQ signal path.
pub fn render_master_section(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    batch: &mut GuiCommandBatch,
) {
    ui.separator();
    ui.heading("Master");

    ui.horizontal(|ui| {
        // Bypass checkbox.
        let bypass_val = snapshot.get(ParamId(1)).unwrap_or(0.0);
        let mut bypass = bypass_val >= 0.5;

        egui::Checkbox::new(&mut bypass, "Bypass")
            .ui(ui)
            .on_hover_text("Bypass the entire EQ");

        if bypass {
            batch.push(ParamId(1), 1.0);
        } else {
            batch.push(ParamId(1), 0.0);
        }

        // Output gain slider.
        let mut output_gain = snapshot.get(ParamId(2)).unwrap_or(0.0);
        egui::Slider::new(&mut output_gain, -24.0..=24.0)
            .suffix(" dB")
            .ui(ui);
        batch.push(ParamId(2), output_gain);

        // Auto-gain checkbox.
        let auto_gain_val = snapshot.get(ParamId(3)).unwrap_or(0.0);
        let mut auto_gain = auto_gain_val >= 0.5;

        egui::Checkbox::new(&mut auto_gain, "Auto Gain")
            .ui(ui)
            .on_hover_text("Enable auto-gain compensation");

        if auto_gain {
            batch.push(ParamId(3), 1.0);
        } else {
            batch.push(ParamId(3), 0.0);
        }
    });

    ui.separator();
}

// ---------------------------------------------------------------------------
// Engine interface contract (implemented by romueq-engine)
// ---------------------------------------------------------------------------

/// The interface the GUI uses to interact with the engine.
///
/// This trait is implemented by romueq-engine and provides a clean boundary:
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
        "RomuEq"
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

// ---------------------------------------------------------------------------
// Standalone entry point (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "standalone")]
use eframe::App;

/// Standalone entry point for RomuEq.
///
/// Creates an egui window backed by winit and runs the GUI application loop.
#[cfg(feature = "standalone")]
pub mod standalone {
    use super::*;
    use romueq_common::state::Preset;

    /// Run the GUI application as a standalone desktop app.
    pub fn run(app: impl GuiApp) -> eframe::Result {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_resizable(true),
            ..Default::default()
        };
        let window_title = app.window_title().to_string();
        eframe::run_native(
            &window_title,
            options,
            Box::new(move |cc| {
                let ppp = cc.egui_ctx.pixels_per_point();
                cc.egui_ctx.set_pixels_per_point(ppp);
                Ok(Box::new(StandaloneRunner::new(app, cc.egui_ctx.clone())))
            }),
        )
    }

    /// A runner that bridges the GuiApp trait with eframe::App.
    struct StandaloneRunner<A: GuiApp> {
        app: A,
        client: EngineClient,
    }

    impl<A: GuiApp> StandaloneRunner<A> {
        fn new(app: A, ctx: egui::Context) -> Self {
            let mut client = EngineClient::new();
            client.set_egui_context(ctx);
            Self { app, client }
        }
    }

    impl<A: GuiApp> App for StandaloneRunner<A> {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            // Update DPI scaling.
            let _pixels_per_point = ctx.pixels_per_point();

            // Query engine state.
            let _snapshot = self.client.snapshot();
            let analysis = self.client.analysis_snapshot();

            // Process any pending commands.
            let batch = self.client.take_pending_commands();
            if !batch.is_empty() {
                self.client.apply_commands(batch.clone());
                // Update band parameters from commands for real-time FR plot.
                self.client.apply_params_from_batch(&batch);
            }

            // Update GUI app with current state.
            self.app.update(&mut self.client);

            // Render the UI.
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("RomuEq — EQ Parameter Editor");

                // Frequency response plot (real DSP data).
                ui.separator();
                render_frequency_response_plot(
                    ui,
                    &self.client.band_params,
                    self.client.sample_rate,
                    512,
                );

                // Spectrum analyzer (FFT data).
                ui.separator();
                render_spectrum_analyzer(ui, &analysis, self.client.sample_rate, 128);

                ui.separator();
                ui.label("Analysis:");
                if analysis.dropped {
                    ui.colored_label(egui::Color32::RED, "Dropped frames detected");
                }
            });

            // Repaint continuously for real-time response.
            ctx.request_repaint();
        }

        fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
            // Light theme background.
            [0.95, 0.95, 0.95, 1.0]
        }
    }

    /// A minimal engine client that satisfies GuiClient for standalone mode.
    ///
    /// In production, this would be wrapped around the actual romueq-engine.
    struct EngineClient {
        snapshot: ParamSnapshot,
        analysis: AnalysisSnapshot,
        pending_commands: GuiCommandBatch,
        preset_manager: PresetManager,
        egui_context: Option<egui::Context>,
        /// Band parameters for the frequency response plot.
        band_params: [romueq_common::BandParameters; romueq_common::MAX_BANDS],
        /// Sample rate for the frequency response plot.
        sample_rate: f32,
    }

    impl EngineClient {
        fn new() -> Self {
            Self {
                snapshot: ParamSnapshot::new(Vec::new()),
                analysis: AnalysisSnapshot::default(),
                pending_commands: GuiCommandBatch::new(),
                preset_manager: PresetManager::new(),
                egui_context: None,
                band_params: core::array::from_fn(|i| romueq_common::BandParameters {
                    frequency_hz: 20.0 * (i + 1) as f32,
                    ..Default::default()
                }),
                sample_rate: 48000.0,
            }
        }

        fn set_egui_context(&mut self, ctx: egui::Context) {
            self.egui_context = Some(ctx);
        }

        fn take_pending_commands(&mut self) -> GuiCommandBatch {
            std::mem::take(&mut self.pending_commands)
        }

        /// Update band parameters from a command batch.
        fn apply_params_from_batch(&mut self, batch: &GuiCommandBatch) {
            let commands = batch.commands();
            for cmd in commands {
                // Map ParamId to band index and parameter
                if let Some(bp) = self
                    .band_params
                    .iter_mut()
                    .find(|b| b.gain_param_id() == cmd.param_id)
                {
                    if cmd.param_id.0 >= 100 && cmd.param_id.0 < 200 {
                        // Gain parameter (offset 3 from band base)
                        bp.gain_db = cmd.value;
                    }
                }
            }
        }
    }

    impl GuiClient for EngineClient {
        fn prepare(&mut self, _sample_rate: f32, _block_size: usize, _channels: usize) {}
        fn process(&mut self, _input: &[f32], _output: &mut [f32]) {}
        fn snapshot(&self) -> ParamSnapshot {
            self.snapshot.clone()
        }
        fn analysis_snapshot(&self) -> AnalysisSnapshot {
            self.analysis.clone()
        }
        fn apply_commands(&mut self, batch: GuiCommandBatch) {
            // Store commands for later processing.
            for cmd in batch.into_commands() {
                self.pending_commands.push(cmd.param_id, cmd.value);
            }
        }
        fn destroy(&mut self) {}
        fn is_prepared(&self) -> bool {
            false // Placeholder
        }

        fn create_preset(&mut self, name: &str) -> Result<String, String> {
            // Create a default preset from the current state.
            let preset = Preset::new(name);
            self.preset_manager
                .add(name.to_string(), preset)
                .map_err(|e| format!("create preset: {e}"))?;
            Ok(name.to_string())
        }

        fn apply_preset(&mut self, name: &str) -> Result<(), String> {
            let preset = self
                .preset_manager
                .get(name)
                .ok_or_else(|| format!("preset '{name}' not found"))?;

            // Apply preset values to the current snapshot.
            for (id, value) in preset.preset.parameter_values.iter() {
                self.snapshot.set(ParamId(id), value);
            }
            Ok(())
        }

        fn list_presets(&self) -> Vec<String> {
            self.preset_manager.iter().map(|p| p.name.clone()).collect()
        }

        fn remove_preset(&mut self, name: &str) -> bool {
            self.preset_manager.remove(name)
        }

        fn save_preset_to_file(&mut self, name: &str, path: &str) -> Result<(), String> {
            self.preset_manager
                .save_to_file(name, path)
                .map_err(|e| format!("save preset: {e}"))
        }

        fn load_preset_from_file(&mut self, path: &str) -> Result<String, String> {
            let named = self
                .preset_manager
                .load_from_file(path)
                .map_err(|e| format!("load preset: {e}"))?;
            Ok(named.name)
        }

        fn egui_context(&self) -> &egui::Context {
            self.egui_context.as_ref().expect("egui context not set")
        }

        fn next_auto_name(&self) -> String {
            let count = self.list_presets().len() + 1;
            format!("Preset {count}")
        }

        fn band_parameters(&self, _band_index: usize) -> Option<BandParameters> {
            // Build BandParameters from the snapshot values for this band.
            let base_id = 100;
            let default_bp = BandParameters::default();
            let gain_db = self
                .snapshot
                .get(ParamId(base_id + 3))
                .unwrap_or(default_bp.gain_db);
            let frequency_hz = self
                .snapshot
                .get(ParamId(base_id + 2))
                .unwrap_or(default_bp.frequency_hz);
            Some(BandParameters {
                gain_db,
                frequency_hz,
                ..default_bp
            })
        }

        fn spectrum_data(&self) -> &[[f64; 2]] {
            &self.analysis.spectrum_data
        }
    }
}

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
}
