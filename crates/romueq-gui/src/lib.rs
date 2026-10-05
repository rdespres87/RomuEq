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
/// before emitting commands through the batch. Only pushes a command when the
/// value actually changes (tracked via egui response.changed()).
pub fn render_band(
    ui: &mut egui::Ui,
    band_index: usize,
    params: &BandParams,
    snapshot: &ParamSnapshot,
    batch: &mut GuiCommandBatch,
) {
    ui.group(|ui| {
        // Band header with enable checkbox.
        let enabled_val = snapshot.get(params.enabled()).unwrap_or(1.0);
        let mut enabled = enabled_val >= 0.5;

        ui.horizontal(|ui| {
            if ui.checkbox(&mut enabled, "").changed() {
                batch.push(params.enabled(), if enabled { 1.0 } else { 0.0 });
            }

            ui.label(params.label());

            // Enable/disable button as alternative.
            if ui
                .button(if enabled { "Disable" } else { "Enable" })
                .clicked()
            {
                let new_val = if enabled { 0.0 } else { 1.0 };
                batch.push(params.enabled(), new_val);
                enabled = !enabled;
            }

            ui.separator();
        });

        // Filter type dropdown (enum, 0–4).
        let ft_val = snapshot.get(params.filter_type()).unwrap_or(2.0);
        let filter_types = ["Low Shelf", "High Shelf", "Bell", "Low Pass", "High Pass"];
        let ft_idx = (ft_val as usize).min(filter_types.len() - 1);
        let mut ft_idx = ft_idx;

        let ft_selected = filter_types[ft_idx].to_string();
        if egui::ComboBox::from_id_salt(format!("ft_{band_index}"))
            .selected_text(ft_selected)
            .show_index(ui, &mut ft_idx, filter_types.len(), |idx| {
                filter_types[idx].to_string()
            })
            .changed()
        {
            batch.push(params.filter_type(), ft_idx as f32);
        }

        // Frequency slider (logarithmic scale, 20–20000 Hz).
        let freq_val = snapshot.get(params.frequency_hz()).unwrap_or(1000.0);
        let freq_min: f32 = 20.0;
        let freq_max: f32 = 20_000.0;
        let mut freq_log = freq_val.log10().clamp(freq_min.log10(), freq_max.log10());

        if ui
            .add(
                egui::Slider::new(&mut freq_log, freq_min.log10()..=freq_max.log10())
                    .logarithmic(true)
                    .suffix(" Hz")
                    .text("Frequency"),
            )
            .changed()
        {
            let new_freq = 10.0_f32.powf(freq_log);
            batch.push(params.frequency_hz(), new_freq);
        }

        // Gain slider (-24 to +24 dB).
        let mut gain_val = snapshot.get(params.gain_db()).unwrap_or(0.0);

        if ui
            .add(
                egui::Slider::new(&mut gain_val, -24.0..=24.0)
                    .suffix(" dB")
                    .text("Gain"),
            )
            .changed()
        {
            batch.push(params.gain_db(), gain_val);
        }

        // Q slider (0.1 to 20).
        let mut q_val = snapshot.get(params.q()).unwrap_or(0.707);

        if ui
            .add(
                egui::Slider::new(&mut q_val, 0.1..=20.0)
                    .suffix(" Q")
                    .text("Q"),
            )
            .changed()
        {
            batch.push(params.q(), q_val);
        }

        // Slope dropdown (enum, 0–3: 6/12/24/48 dB/oct).
        let slope_val = snapshot.get(params.slope()).unwrap_or(1.0);
        let slopes = ["6", "12", "24", "48"];
        let sl_idx = (slope_val as usize).min(slopes.len() - 1);
        let mut sl_idx = sl_idx;

        let sl_selected = format!("{} dB/oct", slopes[sl_idx]);
        if egui::ComboBox::from_id_salt(format!("slope_{band_index}"))
            .selected_text(sl_selected)
            .show_index(ui, &mut sl_idx, slopes.len(), |idx| {
                format!("{} dB/oct", slopes[idx])
            })
            .changed()
        {
            batch.push(params.slope(), sl_idx as f32);
        }
    });
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
        audio_stream: Option<audio::AudioStream>,
        audio_active: bool,
    }

    impl<A: GuiApp> StandaloneRunner<A> {
        fn new(app: A, ctx: egui::Context) -> Self {
            let mut client = EngineClient::new();
            client.set_egui_context(ctx);
            // Initialize the engine at startup so encode_snapshot() returns real data.
            client.prepare(48000.0, 512, 2);
            let audio_stream = Some(audio::AudioStream::new());
            Self {
                app,
                client,
                audio_stream,
                audio_active: false,
            }
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

                // Audio playback controls.
                ui.horizontal(|ui| {
                    if ui
                        .button(if self.audio_active { "Stop" } else { "Play" })
                        .clicked()
                    {
                        if let Some(ref mut audio) = self.audio_stream {
                            if self.audio_active {
                                let _ = audio.stop();
                            } else {
                                // Sync current engine params to audio thread.
                                let params = self
                                    .client
                                    .engine
                                    .as_ref()
                                    .map(|e| *e.eq_parameters())
                                    .unwrap_or_default();
                                let _ = audio.set_parameters(params);
                                let _ = audio.start();
                            }
                            self.audio_active = !self.audio_active;
                        }
                    }
                    ui.label(if self.audio_active {
                        "▶ Audio active"
                    } else {
                        "○ Audio stopped"
                    });
                });

                ui.separator();

                // Build a batch of commands from user interaction.
                let mut batch = GuiCommandBatch::new();

                // Render all 8 band controls.
                let band_controls = BandControls::new();
                for i in 0..romueq_common::MAX_BANDS {
                    let bp = band_controls.params(i);
                    render_band(ui, i, &bp, &self.client.snapshot, &mut batch);
                }

                // Process the batch through the engine client.
                if !batch.is_empty() {
                    self.client.apply_commands(batch.clone());
                    self.client.apply_params_from_batch(&batch);
                }

                // Master section (bypass, output gain, auto-gain).
                render_master_section(ui, &self.client.snapshot, &mut batch);

                // Preset management UI.
                ui.separator();
                ui.horizontal(|ui| {
                    let mut preset_name = String::new();

                    // Text input for preset name.
                    ui.text_edit_singleline(&mut preset_name);

                    if ui.button("Save Preset").clicked() && !preset_name.trim().is_empty() {
                        let name = preset_name.trim().to_string();
                        if let Err(e) = self.client.create_preset(&name) {
                            eprintln!("Failed to create preset '{name}': {e}");
                        }
                        preset_name.clear();
                    }

                    // List of presets from the manager.
                    let presets: Vec<String> = self
                        .client
                        .preset_manager
                        .iter()
                        .map(|p| p.name.clone())
                        .collect();

                    egui::ComboBox::new("preset_select", "")
                        .selected_text(presets.first().map(|s| s.as_str()).unwrap_or("No presets"))
                        .show_ui(ui, |ui| {
                            for name in &presets {
                                ui.selectable_value(
                                    &mut self.client._current_preset,
                                    name.clone(),
                                    name.as_str(),
                                );
                            }
                        });

                    if ui.button("Load Preset").clicked() {
                        let sel = self.client._current_preset.clone();
                        if !sel.is_empty() {
                            let _ = self.client.apply_preset(&sel);
                        }
                    }

                    if ui.button("Remove Preset").clicked() {
                        let rem = self.client._current_preset.clone();
                        if !rem.is_empty() {
                            let _ = self.client.remove_preset(&rem);
                            self.client._current_preset.clear();
                        }
                    }
                });

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

    use romueq_analysis::AnalysisEngine;
    use romueq_engine::EngineLifecycle;

    /// A real engine client that wraps the actual romueq-engine.
    struct EngineClient {
        engine: Option<EngineLifecycle>,
        analysis_engine: AnalysisEngine,
        snapshot: ParamSnapshot,
        pending_commands: GuiCommandBatch,
        preset_manager: PresetManager,
        egui_context: Option<egui::Context>,
        /// Band parameters for the frequency response plot.
        band_params: [romueq_common::BandParameters; romueq_common::MAX_BANDS],
        /// Sample rate for the frequency response plot.
        sample_rate: f32,
        /// Currently selected preset name for the combo box.
        _current_preset: String,
        /// Spectrum processor for real-time FFT visualization.
        _spectrum_processor: romueq_analysis::SpectrumProcessor,
        /// Running sample counter for spectrum frame numbering.
        _spectrum_frame_number: u64,
    }

    impl EngineClient {
        fn new() -> Self {
            Self {
                engine: None,
                analysis_engine: AnalysisEngine::new(),
                snapshot: ParamSnapshot::new(Vec::new()),
                pending_commands: GuiCommandBatch::new(),
                preset_manager: PresetManager::new(),
                egui_context: None,
                band_params: core::array::from_fn(|i| romueq_common::BandParameters {
                    frequency_hz: 20.0 * (i + 1) as f32,
                    ..Default::default()
                }),
                sample_rate: 48000.0,
                _current_preset: String::new(),
                _spectrum_processor: romueq_analysis::SpectrumProcessor::new(
                    romueq_analysis::SpectrumConfig::new(1024, 48000.0),
                ),
                _spectrum_frame_number: 0,
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
                let pid = cmd.param_id.0;
                // Global parameters (1..=99)
                if (1..=99).contains(&pid) {
                    continue;
                }
                if pid >= 100 {
                    let band_idx = ((pid - 100) / 100) as usize;
                    let offset = ((pid - 100) % 100) as usize;
                    if band_idx < romueq_common::MAX_BANDS {
                        let bp = &mut self.band_params[band_idx];
                        match offset {
                            0 => bp.enabled = cmd.value > 0.5,
                            1 => bp.filter_type = Self::filter_type_from_f32(cmd.value),
                            2 => bp.frequency_hz = cmd.value,
                            3 => bp.gain_db = cmd.value.clamp(-24.0, 24.0),
                            4 => bp.q = cmd.value.clamp(0.1, 20.0),
                            5 => bp.slope = Self::slope_from_f32(cmd.value),
                            _ => {}
                        }
                    }
                }
            }
        }

        /// Map a float value to a FilterType enum variant.
        fn filter_type_from_f32(v: f32) -> romueq_common::FilterType {
            match v as i32 {
                0 => romueq_common::FilterType::LowShelf,
                1 => romueq_common::FilterType::HighShelf,
                2 => romueq_common::FilterType::Bell,
                3 => romueq_common::FilterType::LowPass,
                4 => romueq_common::FilterType::HighPass,
                _ => romueq_common::FilterType::Bell,
            }
        }

        /// Map a float value to a Slope enum variant.
        fn slope_from_f32(v: f32) -> romueq_common::Slope {
            match v as i32 {
                0 => romueq_common::Slope::Db6,
                1 => romueq_common::Slope::Db12,
                2 => romueq_common::Slope::Db18,
                3 => romueq_common::Slope::Db24,
                _ => romueq_common::Slope::Db12,
            }
        }

        /// Map a FilterType enum to a float value (for snapshot encoding).
        fn filter_type_to_f32(ft: romueq_common::FilterType) -> f32 {
            match ft {
                romueq_common::FilterType::LowShelf => 0.0,
                romueq_common::FilterType::HighShelf => 1.0,
                romueq_common::FilterType::Bell => 2.0,
                romueq_common::FilterType::LowPass => 3.0,
                romueq_common::FilterType::HighPass => 4.0,
            }
        }

        /// Map a Slope enum to a float value (for snapshot encoding).
        fn slope_to_f32(sl: romueq_common::Slope) -> f32 {
            match sl {
                romueq_common::Slope::Db6 => 0.0,
                romueq_common::Slope::Db12 => 1.0,
                romueq_common::Slope::Db18 => 2.0,
                romueq_common::Slope::Db24 => 3.0,
            }
        }

        /// Map a ProcessingMode enum to a float value (for snapshot encoding).
        fn processing_mode_to_f32(_pm: romueq_common::ProcessingMode) -> f32 {
            0.0
        }

        /// Map a StereoMode enum to a float value (for snapshot encoding).
        fn stereo_mode_to_f32(_sm: romueq_common::StereoMode) -> f32 {
            0.0
        }

        /// Encode the current EqParameters into a ParamSnapshot for widget sync.
        fn encode_snapshot(&self) -> Vec<ParamValue> {
            let mut values: Vec<ParamValue> = Vec::new();

            if let Some(ref eng) = self.engine {
                let params = eng.eq_parameters();

                // Global parameters
                values.push(ParamValue {
                    param_id: ParamId(1),
                    value: if params.global.bypass { 1.0 } else { 0.0 },
                });
                values.push(ParamValue {
                    param_id: ParamId(2),
                    value: params.global.output_gain_db,
                });
                values.push(ParamValue {
                    param_id: ParamId(3),
                    value: if params.global.auto_gain { 1.0 } else { 0.0 },
                });
                values.push(ParamValue {
                    param_id: ParamId(4),
                    value: Self::processing_mode_to_f32(params.global.processing_mode),
                });
                values.push(ParamValue {
                    param_id: ParamId(5),
                    value: Self::stereo_mode_to_f32(params.global.stereo_mode),
                });

                // Band parameters
                for band_idx in 0..romueq_common::MAX_BANDS {
                    let base = 100u32 + band_idx as u32 * 100;
                    let band = &params.bands[band_idx];
                    values.push(ParamValue {
                        param_id: ParamId(base),
                        value: if band.enabled { 1.0 } else { 0.0 },
                    });
                    values.push(ParamValue {
                        param_id: ParamId(base + 1),
                        value: Self::filter_type_to_f32(band.filter_type),
                    });
                    values.push(ParamValue {
                        param_id: ParamId(base + 2),
                        value: band.frequency_hz,
                    });
                    values.push(ParamValue {
                        param_id: ParamId(base + 3),
                        value: band.gain_db,
                    });
                    values.push(ParamValue {
                        param_id: ParamId(base + 4),
                        value: band.q,
                    });
                    values.push(ParamValue {
                        param_id: ParamId(base + 5),
                        value: Self::slope_to_f32(band.slope),
                    });
                }
            }

            values
        }

        /// Apply a single GuiCommand to EqParameters.
        fn apply_command_to_params(params: &mut romueq_common::EqParameters, cmd: &GuiCommand) {
            let pid = cmd.param_id.0;
            if (1..=99).contains(&pid) {
                // Global parameters
                match pid {
                    1 => params.global.bypass = cmd.value > 0.5,
                    2 => params.global.output_gain_db = cmd.value.clamp(-24.0, 24.0),
                    3 => params.global.auto_gain = cmd.value > 0.5,
                    4 => {
                        params.global.processing_mode = romueq_common::ProcessingMode::MinimumPhase;
                    }
                    5 => {
                        params.global.stereo_mode = romueq_common::StereoMode::Stereo;
                    }
                    _ => {}
                }
            } else if pid >= 100 {
                // Band parameters: band_idx = (pid - 100) / 100
                let band_idx = ((pid - 100) / 100) as usize;
                if band_idx >= romueq_common::MAX_BANDS {
                    return;
                }
                let offset = ((pid - 100) % 100) as usize;
                let band = &mut params.bands[band_idx];
                match offset {
                    0 => band.enabled = cmd.value > 0.5,
                    1 => band.filter_type = Self::filter_type_from_f32(cmd.value),
                    2 => band.frequency_hz = cmd.value,
                    3 => band.gain_db = cmd.value.clamp(-24.0, 24.0),
                    4 => band.q = cmd.value.clamp(0.1, 20.0),
                    5 => band.slope = Self::slope_from_f32(cmd.value),
                    _ => {}
                }
            }
        }

        /// Drain analysis frames from the engine into the analysis engine.
        fn drain_analysis_frames(&mut self) {
            if let Some(ref mut eng) = self.engine {
                let abuf = eng.analysis_buffer();
                while let Some(frame) = abuf.pop() {
                    self.analysis_engine.push(frame);
                }
            }
        }
    }

    impl GuiClient for EngineClient {
        fn prepare(&mut self, sample_rate: f32, block_size: usize, channels: usize) {
            // Destroy existing engine if present.
            if let Some(ref mut eng) = self.engine {
                eng.destroy();
            }

            // Create a new engine.
            let mut eng = EngineLifecycle::new();
            if eng.prepare(sample_rate as f64, block_size, channels) {
                self.engine = Some(eng);

                // Reset band_params to defaults.
                for bp in &mut self.band_params {
                    *bp = Default::default();
                }

                // Update snapshot from engine params.
                self.snapshot = ParamSnapshot::new(self.encode_snapshot());
            } else {
                self.engine = None;
            }

            self.sample_rate = sample_rate;
        }

        fn process(&mut self, input: &[f32], output: &mut [f32]) {
            let eng = match self.engine.as_mut() {
                Some(e) => e,
                None => return,
            };

            // Apply pending commands first.
            if !self.pending_commands.is_empty() {
                let batch = std::mem::take(&mut self.pending_commands);
                let mut params = *eng.eq_parameters();
                for cmd in batch.commands() {
                    Self::apply_command_to_params(&mut params, cmd);
                }
                eng.set_parameters(params);
            }

            // Convert flat interleaved → planar for engine.
            let channels = 2; // stereo default
            let block_size = input.len() / channels;
            let input_planar: [&[f32]; 2] = [&input[0..block_size], &input[block_size..]];
            let (lo, hi) = output.split_at_mut(block_size);
            let mut output_planar: [&mut [f32]; 2] = [lo, hi];

            let _ = eng.process(&input_planar, &mut output_planar);

            // Update snapshot and band_params from engine before releasing borrow.
            let eq_params = *eng.eq_parameters();

            // Drain analysis frames.
            self.drain_analysis_frames();

            // Update snapshot from engine params.
            self.snapshot = ParamSnapshot::new(self.encode_snapshot());

            // Update band_params from engine.
            for i in 0..romueq_common::MAX_BANDS {
                self.band_params[i] = eq_params.bands[i];
            }
        }

        fn snapshot(&self) -> ParamSnapshot {
            self.snapshot.clone()
        }

        fn analysis_snapshot(&self) -> AnalysisSnapshot {
            let mut rms = [f32::NEG_INFINITY; romueq_common::ANALYSIS_MAX_CHANNELS];
            let mut peak = [f32::NEG_INFINITY; romueq_common::ANALYSIS_MAX_CHANNELS];
            let mut dropped = false;

            // Pull the latest analysis frame if available.
            if let Some(frame) = self.analysis_engine.latest_snapshot() {
                for (i, &v) in frame.rms.iter().enumerate() {
                    if i < romueq_common::ANALYSIS_MAX_CHANNELS {
                        rms[i] = v;
                        peak[i] = frame.peak[i];
                    }
                }
                dropped = frame.dropped;
            }

            // Generate spectrum data from the current EQ frequency response.
            // Without a real audio input (Phase 6), we render the filter curve
            // as the spectrum so the user sees the EQ shape in real time.
            let mut spectrum = Vec::new();
            let sr = self.sample_rate;
            let nyquist = sr * 0.5;
            let num_bins = 128;
            let log_min = 20.0_f64.log10();
            let log_max = (nyquist as f64).log10();

            // We use the band_params (which get updated from commands) to
            // create a simple synthetic spectrum by evaluating the EQ response.
            for i in 0..num_bins {
                let t = if num_bins == 1 {
                    0.5_f64
                } else {
                    i as f64 / (num_bins - 1) as f64
                };
                let freq = 10.0_f64.powf(log_min + t * (log_max - log_min));

                // Compute magnitude at this frequency from the EQ bands.
                let mut mag_db: f32 = 0.0;
                for bp in &self.band_params {
                    if !bp.enabled {
                        continue;
                    }
                    let freq_f32 = bp.frequency_hz;
                    let gain = bp.gain_db;
                    let ft = bp.filter_type;

                    // Simple approximation: apply the band's gain near its center freq.
                    let ratio = (freq / freq_f32 as f64).log2().abs();
                    let attenuation = match ft {
                        romueq_common::FilterType::Bell => ratio * 2.0,
                        romueq_common::FilterType::LowShelf => {
                            if freq > freq_f32 as f64 {
                                ratio * 0.5
                            } else {
                                0.0
                            }
                        }
                        romueq_common::FilterType::HighShelf => {
                            if freq < freq_f32 as f64 {
                                ratio * 0.5
                            } else {
                                0.0
                            }
                        }
                        romueq_common::FilterType::LowPass => {
                            if freq > freq_f32 as f64 {
                                ratio * 6.0
                            } else {
                                0.0
                            }
                        }
                        romueq_common::FilterType::HighPass => {
                            if freq < freq_f32 as f64 {
                                ratio * 6.0
                            } else {
                                0.0
                            }
                        }
                    };
                    let mag = ((gain as f64) * (1.0 / (1.0 + attenuation))) as f32;
                    mag_db += mag;
                }

                spectrum.push([freq, mag_db as f64]);
            }

            AnalysisSnapshot {
                rms,
                peak,
                dropped,
                spectrum_data: spectrum,
            }
        }

        fn apply_commands(&mut self, batch: GuiCommandBatch) {
            if let Some(ref mut eng) = self.engine {
                let mut params = *eng.eq_parameters();
                for cmd in batch.commands() {
                    Self::apply_command_to_params(&mut params, cmd);
                }
                eng.set_parameters(params);
                // Sync band_params from engine so the UI always reflects engine state.
                let eq_params = *eng.eq_parameters();
                for i in 0..romueq_common::MAX_BANDS {
                    self.band_params[i] = eq_params.bands[i];
                }
                // Update snapshot from engine parameters.
                self.snapshot = ParamSnapshot::new(self.encode_snapshot());
            } else {
                for cmd in batch.commands() {
                    self.pending_commands.push(cmd.param_id, cmd.value);
                }
            }
        }

        fn destroy(&mut self) {
            if let Some(ref mut eng) = self.engine {
                eng.destroy();
            }
            self.engine = None;
        }

        fn is_prepared(&self) -> bool {
            self.engine.is_some()
        }

        fn create_preset(&mut self, name: &str) -> Result<String, String> {
            let _params = self
                .engine
                .as_ref()
                .map(|e| *e.eq_parameters())
                .unwrap_or_default();

            let mut preset = Preset::new(name);
            for pv in self.encode_snapshot().into_iter() {
                preset.parameter_values.set(pv.param_id.0, pv.value);
            }

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

            if let Some(ref mut eng) = self.engine {
                let mut params = *eng.eq_parameters();
                for (id, value) in preset.preset.parameter_values.iter() {
                    let pid = ParamId(id);
                    Self::apply_command_to_params(
                        &mut params,
                        &GuiCommand {
                            param_id: pid,
                            value,
                        },
                    );
                }
                eng.set_parameters(params);
            }

            // Update snapshot and band_params locally.
            self.snapshot = ParamSnapshot::new(self.encode_snapshot());
            let eq_params = self
                .engine
                .as_ref()
                .map(|e| *e.eq_parameters())
                .unwrap_or_default();
            for i in 0..romueq_common::MAX_BANDS {
                self.band_params[i] = eq_params.bands[i];
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

            if let Some(ref mut eng) = self.engine {
                let mut params = *eng.eq_parameters();
                for (id, value) in named.preset.parameter_values.iter() {
                    let pid = ParamId(id);
                    Self::apply_command_to_params(
                        &mut params,
                        &GuiCommand {
                            param_id: pid,
                            value,
                        },
                    );
                }
                eng.set_parameters(params);
            }

            // Update snapshot and band_params locally.
            self.snapshot = ParamSnapshot::new(self.encode_snapshot());
            let eq_params = self
                .engine
                .as_ref()
                .map(|e| *e.eq_parameters())
                .unwrap_or_default();
            for i in 0..romueq_common::MAX_BANDS {
                self.band_params[i] = eq_params.bands[i];
            }

            Ok(named.name)
        }

        fn egui_context(&self) -> &egui::Context {
            self.egui_context.as_ref().expect("egui context not set")
        }

        fn next_auto_name(&self) -> String {
            self.preset_manager.next_auto_name()
        }

        fn band_parameters(&self, band_index: usize) -> Option<BandParameters> {
            if band_index < romueq_common::MAX_BANDS {
                Some(self.band_params[band_index])
            } else {
                None
            }
        }

        fn spectrum_data(&self) -> &[[f64; 2]] {
            // Return a reference to the latest spectrum data.
            // In Phase 6 (audio backend), this will be updated every sample block.
            // For now, return an empty slice — the analysis_snapshot() already
            // provides spectrum_data for the UI plot.
            static EMPTY: &[[f64; 2]] = &[];
            EMPTY
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
        for i in 0..romueq_common::MAX_BANDS {
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
        let channels = romueq_common::ANALYSIS_MAX_CHANNELS;
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
        let _fn: fn(&mut egui::Ui, &[romueq_common::BandParameters; 8], f32, usize) =
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
        for i in 0..romueq_common::MAX_BANDS {
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
        for i in 0..romueq_common::MAX_BANDS {
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
