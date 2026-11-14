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

/// egui Widget trait for `.ui()` method on sliders, checkboxes, etc.
use egui_plot::{GridMark, HLine, Line, Plot, Points, Text, VLine};
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

fn log_freq_grid_spacer(_input: egui_plot::GridInput) -> Vec<GridMark> {
    let freqs: [f64; 10] = [
        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ];
    freqs
        .iter()
        .map(|&f| GridMark {
            value: f.log10(),
            step_size: 1.0,
        })
        .collect()
}

fn log_freq_axis_formatter(mark: GridMark, _range: &std::ops::RangeInclusive<f64>) -> String {
    let freq = 10.0_f64.powf(mark.value);
    if freq >= 990.0 {
        let khz = freq / 1000.0;
        if (khz - khz.round()).abs() < 0.05 {
            format!("{:.0}k", khz.round())
        } else {
            format!("{khz:.1}k")
        }
    } else {
        format!("{freq:.0}")
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
    let band_names = [
        "Sub",
        "Low",
        "Low-Mid",
        "Mid",
        "High-Mid",
        "Presence",
        "Brilliance",
        "Air",
    ];
    let band_name = band_names.get(band_index).copied().unwrap_or("Band");

    let color = match band_index % 8 {
        0 => egui::Color32::from_rgb(255, 80, 80),   // Red
        1 => egui::Color32::from_rgb(255, 160, 40),  // Orange
        2 => egui::Color32::from_rgb(255, 220, 50),  // Yellow
        3 => egui::Color32::from_rgb(50, 220, 100),  // Green
        4 => egui::Color32::from_rgb(40, 200, 220),  // Cyan
        5 => egui::Color32::from_rgb(80, 140, 255),  // Blue
        6 => egui::Color32::from_rgb(180, 90, 255),  // Purple
        _ => egui::Color32::from_rgb(255, 100, 200), // Magenta
    };

    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;

            // 1. Compact single-line colored header badge + ON/OFF toggle
            let enabled_val = snapshot.get(params.enabled()).unwrap_or(1.0);
            let mut enabled = enabled_val >= 0.5;
            let header_color = if enabled {
                color
            } else {
                egui::Color32::from_rgb(110, 115, 125)
            };

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if ui.checkbox(&mut enabled, "").changed() {
                    batch.push(params.enabled(), if enabled { 1.0 } else { 0.0 });
                }
                ui.label(
                    egui::RichText::new(format!("B{}", band_index + 1))
                        .strong()
                        .color(header_color),
                );
                ui.label(
                    egui::RichText::new(band_name)
                        .small()
                        .color(egui::Color32::from_rgb(175, 180, 190)),
                );
            });

            // 2. Compact Filter type dropdown (enum, 0–4 matching FilterType::to_raw)
            let ft_val = snapshot.get(params.filter_type()).unwrap_or(0.0);
            let filter_types = ["Bell", "LowShelf", "HiShelf", "HiPass", "LowPass"];
            let ft_idx = (ft_val.round().max(0.0) as usize).min(filter_types.len() - 1);
            let mut ft_idx = ft_idx;

            let col_w = (ui.available_width() - 4.0).max(52.0);
            egui::ComboBox::from_id_salt(format!("ft_{band_index}"))
                .width(col_w)
                .selected_text(filter_types[ft_idx])
                .show_index(ui, &mut ft_idx, filter_types.len(), |idx| {
                    filter_types[idx].to_string()
                });
            if ft_idx as f32 != ft_val {
                batch.push(params.filter_type(), ft_idx as f32);
            }

            // 3. Vertical Gain Fader (top-to-bottom console strip) + compact dB DragValue
            let mut gain_val = snapshot.get(params.gain_db()).unwrap_or(0.0);
            let is_pass = ft_idx == 3 || ft_idx == 4;
            ui.spacing_mut().slider_width = 68.0;
            if is_pass {
                ui.add_enabled(
                    false,
                    egui::Slider::new(&mut gain_val, -24.0..=24.0)
                        .vertical()
                        .show_value(false),
                );
                ui.add_enabled(
                    false,
                    egui::DragValue::new(&mut gain_val)
                        .range(-24.0..=24.0)
                        .speed(0.1)
                        .fixed_decimals(1)
                        .suffix(" dB"),
                );
            } else {
                if ui
                    .add(
                        egui::Slider::new(&mut gain_val, -24.0..=24.0)
                            .vertical()
                            .show_value(false),
                    )
                    .on_hover_text("Double-click to reset to 0 dB")
                    .changed()
                {
                    batch.push(params.gain_db(), gain_val);
                }
                if ui
                    .add(
                        egui::DragValue::new(&mut gain_val)
                            .range(-24.0..=24.0)
                            .speed(0.1)
                            .fixed_decimals(1)
                            .suffix(" dB"),
                    )
                    .changed()
                {
                    batch.push(params.gain_db(), gain_val);
                }
            }

            // 4. Stacked Frequency DragValue (20–20000 Hz)
            let mut freq_val = snapshot.get(params.frequency_hz()).unwrap_or(1000.0);
            let freq_speed = (freq_val * 0.015).max(1.0);
            if ui
                .add(
                    egui::DragValue::new(&mut freq_val)
                        .range(20.0..=20_000.0)
                        .speed(freq_speed)
                        .custom_formatter(|n, _| {
                            if n >= 1000.0 {
                                format!("{:.2} kHz", n / 1000.0)
                            } else {
                                format!("{n:.0} Hz")
                            }
                        }),
                )
                .on_hover_text("Center frequency (drag or click to edit)")
                .changed()
            {
                batch.push(params.frequency_hz(), freq_val);
            }

            // 5. Stacked Q Factor DragValue (0.1–20.0)
            let mut q_val = snapshot.get(params.q()).unwrap_or(0.707);
            if ui
                .add(
                    egui::DragValue::new(&mut q_val)
                        .range(0.1..=20.0)
                        .speed(0.05)
                        .fixed_decimals(2)
                        .prefix("Q "),
                )
                .on_hover_text("Bandwidth (Q factor)")
                .changed()
            {
                batch.push(params.q(), q_val);
            }

            // 6. Compact Slope dropdown (6/12/24/48 dB/oct)
            let slope_val = snapshot.get(params.slope()).unwrap_or(1.0);
            let slopes = ["6 dB/o", "12 dB/o", "24 dB/o", "48 dB/o"];
            let sl_idx = (slope_val as usize).min(slopes.len() - 1);
            let mut sl_idx = sl_idx;

            egui::ComboBox::from_id_salt(format!("slope_{band_index}"))
                .width(col_w)
                .selected_text(slopes[sl_idx])
                .show_index(ui, &mut sl_idx, slopes.len(), |idx| slopes[idx].to_string());
            if sl_idx as f32 != slope_val {
                batch.push(params.slope(), sl_idx as f32);
            }
        });
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
    bands: &[equaform_common::BandParameters; equaform_common::MAX_BANDS],
    sample_rate: f32,
    width: usize,
) {
    use equaform_dsp::compute_response;

    let response = compute_response(sample_rate, bands, width);

    // Convert to plot points: [log10(frequency_hz), magnitude_db]
    let points: Vec<[f64; 2]> = response
        .iter()
        .map(|p| {
            [
                (p.frequency_hz as f64).max(20.0).log10(),
                p.magnitude_db as f64,
            ]
        })
        .collect();

    let line = Line::new(points)
        .color(egui::Color32::from_rgb(0, 180, 255))
        .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 180, 255)))
        .fill(0.0)
        .name("EQ Response");

    let plot = Plot::new("eq_response")
        .include_x(20.0_f64.log10())
        .include_x(20000.0_f64.log10())
        .include_y(-24.0)
        .include_y(24.0)
        .height(200.0)
        .x_axis_formatter(log_freq_axis_formatter)
        .x_grid_spacer(log_freq_grid_spacer)
        .label_formatter(|_name, val| {
            let freq = 10.0_f64.powf(val.x);
            format!("Freq: {freq:.0} Hz\nGain: {:+.1} dB", val.y)
        })
        .show(ui, |plot_ui| {
            // Subtle 0 dB reference line
            plot_ui.hline(
                HLine::new(0.0)
                    .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                    )),
            );

            plot_ui.line(line);

            // Render interactive visual nodes for each enabled band
            for (i, band) in bands.iter().enumerate() {
                if band.enabled {
                    let color = match i % 8 {
                        0 => egui::Color32::from_rgb(255, 80, 80),   // Red
                        1 => egui::Color32::from_rgb(255, 160, 40),  // Orange
                        2 => egui::Color32::from_rgb(255, 220, 50),  // Yellow
                        3 => egui::Color32::from_rgb(50, 220, 100),  // Green
                        4 => egui::Color32::from_rgb(40, 200, 220),  // Cyan
                        5 => egui::Color32::from_rgb(80, 140, 255),  // Blue
                        6 => egui::Color32::from_rgb(180, 90, 255),  // Purple
                        _ => egui::Color32::from_rgb(255, 100, 200), // Magenta
                    };
                    let node_x = (band.frequency_hz as f64).log10();
                    let node_y = match band.filter_type {
                        equaform_common::FilterType::LowPass
                        | equaform_common::FilterType::HighPass => 0.0,
                        _ => band.gain_db as f64,
                    };
                    plot_ui.points(
                        Points::new(vec![[node_x, node_y]])
                            .color(color)
                            .radius(5.5)
                            .name(format!("Bande {}", i + 1)),
                    );
                    let text_y = if node_y >= 0.0 {
                        node_y + 2.5
                    } else {
                        node_y - 2.5
                    };
                    plot_ui.text(
                        Text::new(
                            egui_plot::PlotPoint::new(node_x, text_y),
                            format!("B{}", i + 1),
                        )
                        .color(color),
                    );
                }
            }
        });

    // Show axis labels
    ui.horizontal(|ui| {
        ui.label("Fréquence (Hz)");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label("Gain (dB)");
        });
    });

    let _ = plot;
}

/// Renders the interactive frequency response plot allowing direct mouse node dragging.
pub fn render_interactive_frequency_response_plot(
    ui: &mut egui::Ui,
    bands: &[equaform_common::BandParameters; equaform_common::MAX_BANDS],
    sample_rate: f32,
    width: usize,
    dragged_band: &mut Option<usize>,
    batch: &mut GuiCommandBatch,
) {
    use equaform_dsp::coefficients::compute_coefficients;
    use equaform_dsp::{biquad_response_at_frequency, compute_response};

    let response = compute_response(sample_rate, bands, width);

    let log_min_x = 16.0_f64.log10();
    let log_max_x = 24000.0_f64.log10();

    let points: Vec<[f64; 2]> = response
        .iter()
        .map(|p| {
            [
                (p.frequency_hz as f64).clamp(20.0, 20000.0).log10(),
                (p.magnitude_db as f64).clamp(-40.0, 40.0),
            ]
        })
        .collect();

    let master_line = Line::new(points)
        .color(egui::Color32::from_rgb(0, 185, 255))
        .stroke(egui::Stroke::new(2.2, egui::Color32::from_rgb(0, 185, 255)))
        .fill(0.0)
        .name("EQ Curve");

    let is_primary_down = ui.ctx().input(|i| i.pointer.primary_down());
    let is_primary_pressed = ui.ctx().input(|i| i.pointer.primary_pressed());
    let mut local_dragged = *dragged_band;
    let mut coord_under_pointer = None;

    let plot = Plot::new("eq_response_interactive")
        .include_x(log_min_x)
        .include_x(log_max_x)
        .include_y(-40.0)
        .include_y(40.0)
        .height(165.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .x_axis_formatter(log_freq_axis_formatter)
        .x_grid_spacer(log_freq_grid_spacer)
        .label_formatter(|_name, val| {
            let freq = 10.0_f64.powf(val.x);
            format!("Freq: {freq:.0} Hz\nGain: {:+.1} dB", val.y)
        })
        .show(ui, |plot_ui| {
            // Lock viewport bounds strictly to [18 Hz .. 22 kHz] x [-40 dB .. +40 dB] so it never slides or clips
            plot_ui.set_auto_bounds(egui::Vec2b::FALSE);
            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                [log_min_x, -40.0],
                [log_max_x, 40.0],
            ));

            // 1. Subtle 0 dB reference line and ±24 dB band limit reference lines
            plot_ui.hline(
                HLine::new(0.0)
                    .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40),
                    )),
            );
            for limit_y in [-24.0, 24.0] {
                plot_ui.hline(
                    HLine::new(limit_y)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 16))
                        .stroke(egui::Stroke::new(
                            0.8,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 16),
                        )),
                );
            }

            // 2. Individual translucent band response curves for active boosted/cut bands
            for (i, band) in bands.iter().enumerate() {
                if band.enabled
                    && (band.gain_db.abs() > 0.05
                        || matches!(
                            band.filter_type,
                            equaform_common::FilterType::LowPass
                                | equaform_common::FilterType::HighPass
                        ))
                {
                    let color = match i % 8 {
                        0 => egui::Color32::from_rgb(255, 80, 80),   // Red
                        1 => egui::Color32::from_rgb(255, 160, 40),  // Orange
                        2 => egui::Color32::from_rgb(255, 220, 50),  // Yellow
                        3 => egui::Color32::from_rgb(50, 220, 100),  // Green
                        4 => egui::Color32::from_rgb(40, 200, 220),  // Cyan
                        5 => egui::Color32::from_rgb(80, 140, 255),  // Blue
                        6 => egui::Color32::from_rgb(180, 90, 255),  // Purple
                        _ => egui::Color32::from_rgb(255, 100, 200), // Magenta
                    };
                    let coeffs = compute_coefficients(
                        band.filter_type,
                        band.frequency_hz,
                        band.gain_db,
                        band.q,
                        band.slope,
                        sample_rate,
                    );
                    let band_curve_points: Vec<[f64; 2]> = (0..width)
                        .map(|k| {
                            let t = k as f32 / (width - 1).max(1) as f32;
                            let log_min = 20.0_f32.log10();
                            let log_max = 20000.0_f32.log10();
                            let f = 10.0_f32.powf(log_min + t * (log_max - log_min));
                            let db = biquad_response_at_frequency(
                                &coeffs,
                                f.min(sample_rate * 0.4999),
                                sample_rate,
                            );
                            [
                                (f as f64).clamp(20.0, 20000.0).log10(),
                                (db as f64).clamp(-40.0, 40.0),
                            ]
                        })
                        .collect();
                    let translucent_color =
                        egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 85);
                    plot_ui.line(
                        Line::new(band_curve_points)
                            .color(translucent_color)
                            .stroke(egui::Stroke::new(1.2, translucent_color)),
                    );
                }
            }

            // 3. Master EQ curve
            plot_ui.line(master_line);

            if let Some(coord) = plot_ui.pointer_coordinate() {
                coord_under_pointer = Some((coord.x, coord.y));
            }

            // 4. Discreet vertical guide lines and small refined studio nodes
            for (i, band) in bands.iter().enumerate() {
                if band.enabled {
                    let color = match i % 8 {
                        0 => egui::Color32::from_rgb(255, 80, 80),   // Red
                        1 => egui::Color32::from_rgb(255, 160, 40),  // Orange
                        2 => egui::Color32::from_rgb(255, 220, 50),  // Yellow
                        3 => egui::Color32::from_rgb(50, 220, 100),  // Green
                        4 => egui::Color32::from_rgb(40, 200, 220),  // Cyan
                        5 => egui::Color32::from_rgb(80, 140, 255),  // Blue
                        6 => egui::Color32::from_rgb(180, 90, 255),  // Purple
                        _ => egui::Color32::from_rgb(255, 100, 200), // Magenta
                    };
                    let node_x = (band.frequency_hz as f64).clamp(20.0, 20000.0).log10();
                    let node_y = match band.filter_type {
                        equaform_common::FilterType::LowPass
                        | equaform_common::FilterType::HighPass => 0.0,
                        _ => (band.gain_db as f64).clamp(-24.0, 24.0),
                    };

                    // Subtle vertical guide line
                    let guide_color =
                        egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 32);
                    plot_ui.vline(
                        VLine::new(node_x)
                            .color(guide_color)
                            .stroke(egui::Stroke::new(1.0, guide_color)),
                    );

                    let is_active_node = local_dragged == Some(i);
                    let halo_color = egui::Color32::from_rgba_unmultiplied(
                        color.r(),
                        color.g(),
                        color.b(),
                        if is_active_node { 95 } else { 50 },
                    );
                    let halo_radius = if is_active_node { 5.2 } else { 3.8 };
                    let dot_radius = if is_active_node { 3.6 } else { 2.5 };

                    plot_ui.points(
                        Points::new(vec![[node_x, node_y]])
                            .color(halo_color)
                            .radius(halo_radius),
                    );
                    plot_ui.points(
                        Points::new(vec![[node_x, node_y]])
                            .color(color)
                            .radius(dot_radius)
                            .name(format!("Bande {}", i + 1)),
                    );

                    if is_active_node {
                        let text_y = if node_y >= 0.0 {
                            (node_y + 3.6).min(36.0)
                        } else {
                            (node_y - 3.6).max(-36.0)
                        };
                        plot_ui.text(
                            Text::new(
                                egui_plot::PlotPoint::new(node_x.clamp(1.45, 4.15), text_y),
                                format!(
                                    "B{} ({:.0} Hz, {:+.1} dB)",
                                    i + 1,
                                    band.frequency_hz,
                                    band.gain_db
                                ),
                            )
                            .color(color),
                        );
                    }
                }
            }
        });

    let plot_hovered = plot.response.hovered();
    let plot_rect = plot.response.rect;
    let plot_w = plot_rect.width().max(100.0) as f64;
    let plot_h = plot_rect.height().max(50.0) as f64;

    if is_primary_down {
        if let Some((px, py)) = coord_under_pointer {
            // Only grab a node when pressing inside the plot
            if local_dragged.is_none() && is_primary_pressed && plot_hovered {
                let mut closest = None;
                let mut min_dist_px = f64::MAX;
                for (i, band) in bands.iter().enumerate() {
                    if band.enabled {
                        let node_x = (band.frequency_hz as f64).clamp(20.0, 20000.0).log10();
                        let node_y = match band.filter_type {
                            equaform_common::FilterType::LowPass
                            | equaform_common::FilterType::HighPass => 0.0,
                            _ => (band.gain_db as f64).clamp(-24.0, 24.0),
                        };
                        // Calculate exact pixel distance using the plot widget's actual screen dimensions
                        let dx_px = ((px - node_x) / (log_max_x - log_min_x)) * plot_w;
                        let dy_px = ((py - node_y) / 80.0) * plot_h;
                        let dist_px_sq = dx_px * dx_px + dy_px * dy_px;
                        // Grab radius of 16 screen points (prevents hijacking adjacent bands)
                        if dist_px_sq < 256.0 && dist_px_sq < min_dist_px {
                            min_dist_px = dist_px_sq;
                            closest = Some(i);
                        }
                    }
                }
                local_dragged = closest;
            }

            if let Some(band_idx) = local_dragged {
                let params = BandParams::new(band_idx, (band_idx as u32 + 1) * 100);
                let new_freq =
                    (10.0_f64.powf(px.clamp(log_min_x, log_max_x)) as f32).clamp(20.0, 20_000.0);
                let new_gain = (py as f32).clamp(-24.0, 24.0);
                batch.push(params.frequency_hz(), new_freq);
                match bands[band_idx].filter_type {
                    equaform_common::FilterType::LowPass
                    | equaform_common::FilterType::HighPass => {}
                    _ => batch.push(params.gain_db(), new_gain),
                }
            }
        }
    } else {
        local_dragged = None;
    }

    // Handle mouse scroll wheel to adjust Q factor of the hovered or dragged band
    let scroll_delta = ui.ctx().input(|i| i.raw_scroll_delta.y);
    if scroll_delta.abs() > 0.1 && (plot_hovered || local_dragged.is_some()) {
        let target_band = local_dragged.or_else(|| {
            if let Some((px, py)) = coord_under_pointer {
                let mut closest = None;
                let mut min_dist_px = f64::MAX;
                for (i, band) in bands.iter().enumerate() {
                    if band.enabled {
                        let node_x = (band.frequency_hz as f64).clamp(20.0, 20000.0).log10();
                        let node_y = match band.filter_type {
                            equaform_common::FilterType::LowPass
                            | equaform_common::FilterType::HighPass => 0.0,
                            _ => (band.gain_db as f64).clamp(-24.0, 24.0),
                        };
                        let dx_px = ((px - node_x) / (log_max_x - log_min_x)) * plot_w;
                        let dy_px = ((py - node_y) / 80.0) * plot_h;
                        let dist_px_sq = dx_px * dx_px + dy_px * dy_px;
                        // Hover radius of 20 screen points for scroll wheel
                        if dist_px_sq < 400.0 && dist_px_sq < min_dist_px {
                            min_dist_px = dist_px_sq;
                            closest = Some(i);
                        }
                    }
                }
                closest
            } else {
                None
            }
        });

        if let Some(band_idx) = target_band {
            let params = BandParams::new(band_idx, (band_idx as u32 + 1) * 100);
            let current_q = bands[band_idx].q;
            // Scroll up -> narrower / higher Q; Scroll down -> wider / lower Q
            let factor = if scroll_delta > 0.0 { 1.10 } else { 0.90 };
            let new_q = (current_q * factor).clamp(0.1, 30.0);
            batch.push(params.q(), new_q);
        }
    }

    *dragged_band = local_dragged;

    let _ = plot;
}

// ---------------------------------------------------------------------------
// Spectrum analyzer widget
// ---------------------------------------------------------------------------

/// Renders a real-time spectrum analyzer from analysis data.
///
/// Displays the latest FFT magnitude data as a log-scaled bar chart.
/// Uses `analysis.spectrum_data` (a Vec of [freq_hz, magnitude_db] pairs)
/// when available; falls back to a flat line at -90 dB when no data exists.
pub fn render_spectrum_analyzer(
    ui: &mut egui::Ui,
    analysis: &AnalysisSnapshot,
    sample_rate: f32,
    num_bars: usize,
) {
    let nyquist = sample_rate * 0.5;

    // Build frequency bins (logarithmically spaced) for x-axis mapping.
    let log_min = 16.0_f64.log10();
    let log_max = (nyquist as f64).min(24000.0).log10();

    let mut bars: Vec<[f64; 2]> = Vec::with_capacity(num_bars);

    if analysis.spectrum_data.is_empty() {
        // No spectrum data yet — draw a flat floor at -90 dB.
        for i in 0..num_bars {
            let t = if num_bars == 1 {
                0.5_f64
            } else {
                i as f64 / (num_bars - 1) as f64
            };
            let log_freq = log_min + t * (log_max - log_min);
            bars.push([log_freq, -90.0]);
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

            let clamped_db = mag_db.clamp(-90.0, 0.0);
            bars.push([target_freq.log10(), clamped_db]);
        }
    }

    let bar_chart = Line::new(bars)
        .color(egui::Color32::from_rgb(0, 230, 180)) // Studio cyan/emerald
        .stroke(egui::Stroke::new(1.8, egui::Color32::from_rgb(0, 230, 180)))
        .fill(-90.0)
        .name("Spectrum");

    let plot = Plot::new("spectrum")
        .include_x(log_min)
        .include_x(log_max)
        .include_y(-90.0)
        .include_y(0.0)
        .height(165.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .x_axis_formatter(log_freq_axis_formatter)
        .x_grid_spacer(log_freq_grid_spacer)
        .label_formatter(|_name, val| {
            let freq = 10.0_f64.powf(val.x);
            format!("Freq: {freq:.0} Hz\nLevel: {:+.1} dBFS", val.y)
        })
        .show(ui, |plot_ui| {
            plot_ui.set_auto_bounds(egui::Vec2b::FALSE);
            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                [log_min, -90.0],
                [log_max, 0.0],
            ));
            plot_ui.line(bar_chart);
        });

    let _ = plot;
}

// ---------------------------------------------------------------------------
// Master section controls widget
// ---------------------------------------------------------------------------

/// Renders a graphical Stereo Left/Right VU-Meter (-60 dBFS to 0 dBFS) with studio color zones.
pub fn render_stereo_vu_meter(ui: &mut egui::Ui, peak_l_db: f32, peak_r_db: f32) {
    let draw_channel = |ui: &mut egui::Ui, label: &str, db: f32| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                egui::RichText::new(label)
                    .small()
                    .strong()
                    .color(egui::Color32::from_rgb(180, 185, 195)),
            );

            let bar_w = 130.0;
            let bar_h = 8.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, bar_h), egui::Sense::hover());

            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                // Dark background track
                painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(28, 30, 36));

                // Normalized fill from -60 dB .. 0 dB
                let norm = ((db.clamp(-60.0, 0.0) + 60.0) / 60.0).clamp(0.0, 1.0);
                if norm > 0.005 {
                    // Green segment (-60 to -12 dB -> 0.0 to 0.80)
                    let green_end = norm.min(0.80);
                    let green_rect = egui::Rect::from_min_max(
                        rect.min,
                        egui::pos2(rect.min.x + bar_w * green_end, rect.max.y),
                    );
                    painter.rect_filled(green_rect, 2.0, egui::Color32::from_rgb(45, 215, 115));

                    // Amber segment (-12 to -3 dB -> 0.80 to 0.95)
                    if norm > 0.80 {
                        let amber_end = norm.min(0.95);
                        let amber_rect = egui::Rect::from_min_max(
                            egui::pos2(rect.min.x + bar_w * 0.80, rect.min.y),
                            egui::pos2(rect.min.x + bar_w * amber_end, rect.max.y),
                        );
                        painter.rect_filled(amber_rect, 1.0, egui::Color32::from_rgb(245, 195, 50));
                    }

                    // Red clip segment (-3 to 0 dB -> 0.95 to 1.0)
                    if norm > 0.95 {
                        let red_rect = egui::Rect::from_min_max(
                            egui::pos2(rect.min.x + bar_w * 0.95, rect.min.y),
                            egui::pos2(rect.min.x + bar_w * norm, rect.max.y),
                        );
                        painter.rect_filled(red_rect, 1.0, egui::Color32::from_rgb(245, 70, 70));
                    }
                }

                // Subtle border
                painter.rect_stroke(
                    rect,
                    2.0,
                    egui::Stroke::new(0.8, egui::Color32::from_rgb(60, 65, 75)),
                    egui::StrokeKind::Inside,
                );
            }

            let db_text = if db <= -90.0 {
                "-∞ dB".to_string()
            } else {
                format!("{db:+5.1} dB")
            };
            ui.label(
                egui::RichText::new(db_text)
                    .small()
                    .monospace()
                    .color(if db > -1.0 {
                        egui::Color32::from_rgb(255, 90, 90)
                    } else {
                        egui::Color32::from_rgb(165, 170, 180)
                    }),
            );
        });
    };

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        draw_channel(ui, "L", peak_l_db);
        draw_channel(ui, "R", peak_r_db);
    });
}

/// Renders a vertical studio stereo VU-meter (L / R) with peak hold and dBFS scale.
pub fn render_vertical_stereo_vu_meter(
    ui: &mut egui::Ui,
    peak_l_db: f32,
    peak_r_db: f32,
    height: f32,
) {
    let bar_w = 7.0;
    let gap = 3.0;
    let label_w = 26.0;
    let total_w = bar_w * 2.0 + gap + label_w + 4.0;

    let (rect, _) = ui.allocate_exact_size(egui::vec2(total_w, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();
    let meter_top = rect.min.y + 14.0;
    let meter_bottom = rect.max.y - 4.0;
    let meter_h = (meter_bottom - meter_top).max(20.0);

    // Labels at top
    let l_center_x = rect.min.x + bar_w * 0.5;
    let r_center_x = rect.min.x + bar_w + gap + bar_w * 0.5;
    painter.text(
        egui::pos2(l_center_x, rect.min.y + 5.0),
        egui::Align2::CENTER_CENTER,
        "L",
        egui::FontId::monospace(9.0),
        egui::Color32::from_rgb(170, 180, 195),
    );
    painter.text(
        egui::pos2(r_center_x, rect.min.y + 5.0),
        egui::Align2::CENTER_CENTER,
        "R",
        egui::FontId::monospace(9.0),
        egui::Color32::from_rgb(170, 180, 195),
    );

    // dB marks to render along the right side: +6, 0, -6, -12, -24, -40, -60
    let db_marks: &[(f32, &str)] = &[
        (6.0, "+6"),
        (0.0, "0"),
        (-6.0, "-6"),
        (-12.0, "-12"),
        (-24.0, "-24"),
        (-40.0, "-40"),
        (-60.0, "-60"),
    ];

    let db_to_y = |db: f32| -> f32 {
        let norm = ((db.clamp(-60.0, 6.0) + 60.0) / 66.0).clamp(0.0, 1.0);
        meter_bottom - norm * meter_h
    };

    let text_x = rect.min.x + bar_w * 2.0 + gap + 4.0;
    for &(db, text) in db_marks {
        let y = db_to_y(db);
        let color = if db > 0.0 {
            egui::Color32::from_rgb(250, 90, 90)
        } else if db == 0.0 {
            egui::Color32::from_rgb(240, 240, 240)
        } else {
            egui::Color32::from_rgb(120, 130, 145)
        };
        painter.text(
            egui::pos2(text_x, y),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::monospace(8.0),
            color,
        );
        painter.line_segment(
            [
                egui::pos2(rect.min.x + bar_w * 2.0 + gap, y),
                egui::pos2(rect.min.x + bar_w * 2.0 + gap + 2.0, y),
            ],
            egui::Stroke::new(0.6, egui::Color32::from_rgb(60, 65, 75)),
        );
    }

    // Draw meter channels
    let draw_channel = |x_min: f32, db: f32| {
        let channel_rect = egui::Rect::from_min_max(
            egui::pos2(x_min, meter_top),
            egui::pos2(x_min + bar_w, meter_bottom),
        );
        painter.rect_filled(channel_rect, 1.5, egui::Color32::from_rgb(24, 26, 32));

        let norm = ((db.clamp(-60.0, 6.0) + 60.0) / 66.0).clamp(0.0, 1.0);
        if norm > 0.005 {
            let fill_top = meter_bottom - norm * meter_h;
            // Green segment: up to -12 dB (norm = 48/66 = ~0.727)
            let green_norm = norm.min(48.0 / 66.0);
            let green_top = meter_bottom - green_norm * meter_h;
            let green_rect = egui::Rect::from_min_max(
                egui::pos2(x_min, green_top),
                egui::pos2(x_min + bar_w, meter_bottom),
            );
            painter.rect_filled(green_rect, 1.5, egui::Color32::from_rgb(45, 215, 115));

            // Amber segment: -12 dB to -3 dB (norm 48/66 to 57/66)
            if norm > 48.0 / 66.0 {
                let amber_norm = norm.min(57.0 / 66.0);
                let amber_top = meter_bottom - amber_norm * meter_h;
                let amber_rect = egui::Rect::from_min_max(
                    egui::pos2(x_min, amber_top),
                    egui::pos2(x_min + bar_w, meter_bottom - (48.0 / 66.0) * meter_h),
                );
                painter.rect_filled(amber_rect, 1.0, egui::Color32::from_rgb(245, 195, 50));
            }

            // Red segment: > -3 dB
            if norm > 57.0 / 66.0 {
                let red_rect = egui::Rect::from_min_max(
                    egui::pos2(x_min, fill_top),
                    egui::pos2(x_min + bar_w, meter_bottom - (57.0 / 66.0) * meter_h),
                );
                painter.rect_filled(red_rect, 1.0, egui::Color32::from_rgb(250, 70, 70));
            }
        }

        // Clip LED indicator at very top if clipping (>= -0.1 dB)
        if db >= -0.1 {
            let clip_rect = egui::Rect::from_min_max(
                egui::pos2(x_min, meter_top - 3.0),
                egui::pos2(x_min + bar_w, meter_top),
            );
            painter.rect_filled(clip_rect, 1.0, egui::Color32::from_rgb(255, 40, 40));
        }

        painter.rect_stroke(
            channel_rect,
            1.5,
            egui::Stroke::new(0.7, egui::Color32::from_rgb(55, 60, 70)),
            egui::StrokeKind::Inside,
        );
    };

    draw_channel(rect.min.x, peak_l_db);
    draw_channel(rect.min.x + bar_w + gap, peak_r_db);
}

/// Renders the master section controls: bypass, output gain, and auto-gain.
///
/// These are global parameters that affect the entire EQ signal path.
pub fn render_master_section(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    batch: &mut GuiCommandBatch,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Master:").strong());

        // Output gain slider.
        let mut output_gain = snapshot.get(ParamId(2)).unwrap_or(0.0);
        if ui
            .add(
                egui::Slider::new(&mut output_gain, -24.0..=24.0)
                    .suffix(" dB")
                    .text("Output"),
            )
            .changed()
        {
            batch.push(ParamId(2), output_gain);
        }

        // Auto-gain checkbox.
        let auto_gain_val = snapshot.get(ParamId(3)).unwrap_or(0.0);
        let mut auto_gain = auto_gain_val >= 0.5;

        let auto_gain_label = if auto_gain {
            let comp = snapshot
                .get(ParamId(equaform_common::global::AUTO_GAIN_COMPENSATION_DB))
                .unwrap_or(0.0);
            format!("Auto Gain ({comp:+.1} dB)")
        } else {
            "Auto Gain".to_string()
        };

        if ui
            .checkbox(&mut auto_gain, auto_gain_label)
            .on_hover_text("Automatic perceptual gain compensation")
            .changed()
        {
            batch.push(ParamId(3), if auto_gain { 1.0 } else { 0.0 });
        }
    });
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

#[cfg(feature = "standalone")]
use eframe::App;

/// Standalone entry point for Equaform.
///
/// Creates an egui window backed by winit and runs the GUI application loop.
#[cfg(feature = "standalone")]
pub mod standalone {
    use super::*;
    use equaform_common::state::Preset;

    /// Run the GUI application as a standalone desktop app.
    pub fn run(app: impl GuiApp) -> eframe::Result {
        let (w, h) = app.window_size();
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([w, h])
                .with_min_inner_size([960.0, 475.0])
                .with_resizable(true),
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
        available_outputs: Vec<String>,
        available_inputs: Vec<String>,
        ui_state: WorkspaceUiState,
        spectrum_processor: equaform_analysis::SpectrumProcessor,
        spectrum_frame_num: u64,
        status_message: Option<String>,
    }

    impl<A: GuiApp> StandaloneRunner<A> {
        fn new(app: A, ctx: egui::Context) -> Self {
            let mut client = EngineClient::new();
            client.set_egui_context(ctx);
            // Initialize the engine at startup so encode_snapshot() returns real data.
            client.prepare(48000.0, 512, 2);

            // Populate built-in presets, load user presets from disk, and restore full saved state BEFORE starting audio
            populate_factory_presets(&mut client);

            let mut audio_stream = audio::AudioStream::new();

            // Auto-detect audio devices for seamless macOS EQ replacement
            let in_devices = audio::AudioStream::available_input_devices();
            let out_devices = audio::AudioStream::available_output_devices();

            // Detect virtual input bridge (Equaform prioritized, then legacy RomuEq)
            let detected_in = in_devices
                .iter()
                .find(|d| d.contains("Equaform"))
                .or_else(|| in_devices.iter().find(|d| d.contains("RomuEq")))
                .cloned();

            // Detect real hardware output (restore last selected device if available, else built-in / external)
            let saved_out = load_saved_output_device();
            let detected_out = saved_out
                .and_then(|s| out_devices.iter().find(|d| **d == s).cloned())
                .or_else(|| {
                    out_devices
                        .iter()
                        .find(|d| {
                            let l = d.to_lowercase();
                            l.contains("haut-parleurs")
                                || l.contains("macbook")
                                || l.contains("internal")
                                || l.contains("built-in")
                        })
                        .cloned()
                })
                .or_else(|| {
                    out_devices
                        .iter()
                        .find(|d| {
                            let l = d.to_lowercase();
                            !l.contains("equaform")
                                && !l.contains("romueq")
                        })
                        .cloned()
                })
                .or_else(|| out_devices.first().cloned());

            let initial_mode = audio::AudioSourceMode::PassThrough;

            audio_stream.set_source_mode(initial_mode);
            audio_stream.set_input_device(detected_in.clone());
            audio_stream.set_output_device(detected_out.clone());
            if let Some(ref eng) = client.engine {
                let _ = audio_stream.set_parameters(*eng.eq_parameters());
            }

            let (audio_active, status_message) = match audio_stream.start() {
                Ok(()) => {
                    client.sample_rate = audio_stream.sample_rate();
                    let out_name = detected_out.as_deref().unwrap_or("Output");
                    (true, Some(format!("EQ active → {out_name}")))
                }
                Err(e) => (false, Some(format!("Stream startup error: {e}"))),
            };

            let spectrum_config = equaform_analysis::SpectrumConfig::new(1024, 48000.0);
            let spectrum_processor = equaform_analysis::SpectrumProcessor::new(spectrum_config);

            Self {
                app,
                client,
                audio_stream: Some(audio_stream),
                audio_active,
                available_outputs: out_devices,
                available_inputs: in_devices,
                ui_state: WorkspaceUiState::default(),
                spectrum_processor,
                spectrum_frame_num: 0,
                status_message,
            }
        }
    }

    impl<A: GuiApp> App for StandaloneRunner<A> {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            ctx.set_visuals(egui::Visuals::dark());

            // Update DPI scaling.
            let _pixels_per_point = ctx.pixels_per_point();

            // Query engine state.
            let _snapshot = self.client.snapshot();
            let mut analysis = self.client.analysis_snapshot();

            // Compute real FFT spectrum from audio stream if running
            if self.audio_active {
                if let Some(ref audio) = self.audio_stream {
                    let samples = audio.get_latest_output_samples();
                    if !samples.is_empty() {
                        let sr = audio.sample_rate();
                        let frame = self.spectrum_processor.process(
                            &samples,
                            sr as f64,
                            self.spectrum_frame_num,
                        );
                        self.spectrum_frame_num += 1;
                        // Subtract unnormalized DFT gain 20*log10(N/2) (~54.18 dB for N=1024) so 1.0 full-scale = 0 dBFS
                        let dft_norm_db = 20.0 * ((frame.fft_size as f64) * 0.5).log10();
                        analysis.spectrum_data = (0..frame.num_bins)
                            .map(|b| {
                                [
                                    frame.bin_frequency(b),
                                    (frame.magnitudes_db[b] as f64) - dft_norm_db,
                                ]
                            })
                            .collect();
                    }
                }
            } else {
                analysis.spectrum_data.clear();
            }

            // Process any pending commands from client.
            let pending = self.client.take_pending_commands();
            if !pending.is_empty() {
                self.client.apply_commands(pending);
                // Synchronize live parameters with audio thread
                if let Some(ref mut audio) = self.audio_stream {
                    if let Some(ref engine) = self.client.engine {
                        let _ = audio.set_parameters(*engine.eq_parameters());
                    }
                }
            }

            // Update GUI app with current state.
            self.app.update(&mut self.client);

            // Render the UI.
            // Render the unified workspace UI across Standalone and Plugin modes.
            let env = HostEnvironment::Standalone {
                audio_active: self.audio_active,
                current_output: self
                    .audio_stream
                    .as_ref()
                    .and_then(|a| a.selected_output_device())
                    .unwrap_or("Défaut Système")
                    .to_string(),
                available_outputs: self.available_outputs.clone(),
                peak_levels_db: self
                    .audio_stream
                    .as_ref()
                    .map(|a| a.peak_levels_db())
                    .unwrap_or((-100.0, -100.0)),
            };

            let mut batch = GuiCommandBatch::new();
            let mut actions = Vec::new();

            egui::CentralPanel::default().show(ctx, |ui| {
                let (b, a) = render_workspace(
                    ui,
                    &mut self.ui_state,
                    &env,
                    &self.client.band_params,
                    self.client.sample_rate,
                    &self.client.snapshot,
                    &analysis,
                    &self.client.preset_manager,
                    &self.client._current_preset,
                    self.status_message.as_deref(),
                );
                batch = b;
                actions = a;
            });

            // Handle actions from workspace UI
            for action in actions {
                match action {
                    WorkspaceAction::TogglePlayStop => {
                        if let Some(ref mut audio) = self.audio_stream {
                            if self.audio_active {
                                let _ = audio.stop();
                                self.audio_active = false;
                                self.status_message = Some("Audio muted".to_string());
                            } else {
                                if let Some(ref engine) = self.client.engine {
                                    let _ = audio.set_parameters(*engine.eq_parameters());
                                }
                                match audio.start() {
                                    Ok(()) => {
                                        self.audio_active = true;
                                        self.client.sample_rate = audio.sample_rate();
                                        let out_name =
                                            audio.selected_output_device().unwrap_or("Output");
                                        self.status_message =
                                            Some(format!("EQ active → {out_name}"));
                                    }
                                    Err(err) => {
                                        self.status_message = Some(format!("Audio error: {err}"));
                                    }
                                }
                            }
                        }
                    }
                    WorkspaceAction::SelectOutputDevice(dev) => {
                        if let Some(ref mut audio) = self.audio_stream {
                            audio.set_output_device(Some(dev.clone()));
                            save_selected_output_device(&dev);
                            if let Some(ref engine) = self.client.engine {
                                let _ = audio.set_parameters(*engine.eq_parameters());
                            }
                            if self.audio_active {
                                let _ = audio.stop();
                                match audio.start() {
                                    Ok(()) => {
                                        self.audio_active = true;
                                        self.client.sample_rate = audio.sample_rate();
                                        self.status_message = Some(format!("EQ active → {dev}"));
                                    }
                                    Err(e) => {
                                        self.audio_active = false;
                                        self.status_message =
                                            Some(format!("Output error '{dev}': {e}"));
                                    }
                                }
                            } else {
                                self.status_message = Some(format!("Selected output: {dev}"));
                            }
                        }
                    }
                    WorkspaceAction::RefreshDevices => {
                        self.available_outputs = audio::AudioStream::available_output_devices();
                        self.available_inputs = audio::AudioStream::available_input_devices();
                        self.status_message = Some("Audio devices refreshed".to_string());
                    }
                    WorkspaceAction::ApplyPreset(name) => {
                        let _ = self.client.apply_preset(&name);
                        self.client._current_preset = name.clone();
                        let mut engage = GuiCommandBatch::new();
                        engage.push(ParamId(1), 0.0);
                        self.client.apply_commands(engage);
                        save_last_active_preset_name(&name);
                        save_complete_app_state(&self.client);
                        if let Some(ref mut audio) = self.audio_stream {
                            if let Some(ref engine) = self.client.engine {
                                let _ = audio.set_parameters(*engine.eq_parameters());
                            }
                        }
                        self.status_message = Some(format!("Preset '{name}' applied"));
                    }
                    WorkspaceAction::SavePreset(name) => {
                        if let Err(e) = self.client.create_preset(&name) {
                            self.status_message = Some(format!("Preset error '{name}': {e}"));
                        } else {
                            self.client._current_preset = name.clone();
                            let _ = persist_preset_to_disk(&mut self.client.preset_manager, &name);
                            save_last_active_preset_name(&name);
                            save_complete_app_state(&self.client);
                            self.status_message = Some(format!("Preset '{name}' saved to disk"));
                        }
                    }
                    WorkspaceAction::DeletePreset(name) => {
                        let _ = self.client.remove_preset(&name);
                        remove_preset_from_disk(&name);
                        self.client._current_preset.clear();
                        save_complete_app_state(&self.client);
                        self.status_message = Some(format!("Preset '{name}' deleted"));
                    }
                }
            }

            // Process the batch through the engine client.
            if !batch.is_empty() {
                let modifies_band = batch.commands().iter().any(|c| c.param_id.0 >= 100);
                let modifies_bypass = batch.commands().iter().any(|c| c.param_id.0 == 1);
                if modifies_band && !modifies_bypass {
                    let cur_bypass = self
                        .client
                        .snapshot()
                        .get(ParamId(1))
                        .map(|v| v >= 0.5)
                        .unwrap_or(false);
                    if cur_bypass {
                        batch.push(ParamId(1), 0.0);
                    }
                }
                self.client.apply_commands(batch);
                save_complete_app_state(&self.client);
                if let Some(ref mut audio) = self.audio_stream {
                    if let Some(ref engine) = self.client.engine {
                        let _ = audio.set_parameters(*engine.eq_parameters());
                    }
                }
            }

            // Guarantee that the real-time audio thread always has the exact latest EqParameters
            if let Some(ref mut audio) = self.audio_stream {
                if let Some(ref engine) = self.client.engine {
                    audio.ensure_parameters_synced(*engine.eq_parameters());
                }
            }

            // Repaint continuously for real-time response.
            ctx.request_repaint();
        }

        fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
            save_complete_app_state(&self.client);
            if !self.client._current_preset.is_empty() {
                save_last_active_preset_name(&self.client._current_preset);
            }
            if let Some(ref mut audio) = self.audio_stream {
                let _ = audio.stop();
                audio.restore_system_output_to_selected();
            }
        }

        fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
            // Sleek studio dark background
            [0.10, 0.11, 0.13, 1.0]
        }
    }

    fn user_config_dir() -> std::path::PathBuf {
        crate::presets_io::user_config_dir()
    }

    fn save_selected_output_device(dev: &str) {
        let path = user_config_dir().join("last_output_device.txt");
        let _ = std::fs::write(path, dev.as_bytes());
    }

    fn load_saved_output_device() -> Option<String> {
        let path = user_config_dir().join("last_output_device.txt");
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn save_complete_app_state(client: &EngineClient) {
        let mut state = equaform_common::state::ProjectState::new();
        for pv in client.encode_snapshot() {
            if pv.param_id.0 == 6 {
                continue;
            }
            state.set(pv.param_id.0, pv.value);
        }
        let bytes = state.serialize();
        let path = user_config_dir().join("app_state.bin");
        let _ = std::fs::write(path, bytes);
    }

    fn load_complete_app_state(client: &mut EngineClient) -> bool {
        let path = user_config_dir().join("app_state.bin");
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let Ok(state) = equaform_common::state::ProjectState::deserialize(&bytes) else {
            return false;
        };
        if let Some(ref mut eng) = client.engine {
            let mut params = *eng.eq_parameters();
            for (id, value) in state.parameter_values.iter() {
                if id == 6 {
                    continue;
                }
                EngineClient::apply_command_to_params(
                    &mut params,
                    &GuiCommand {
                        param_id: ParamId(id),
                        value,
                    },
                );
            }
            eng.set_parameters(params);
        }
        client.snapshot = ParamSnapshot::new(client.encode_snapshot());
        let eq_params = client
            .engine
            .as_ref()
            .map(|e| *e.eq_parameters())
            .unwrap_or_default();
        for i in 0..equaform_common::MAX_BANDS {
            client.band_params[i] = eq_params.bands[i];
        }
        true
    }

    fn populate_factory_presets(client: &mut EngineClient) {
        let mut add = |name: &str, configure: fn(&mut equaform_common::EqParameters)| {
            if let Some(ref mut eng) = client.engine {
                let mut params = equaform_common::EqParameters {
                    bands: default_8_bands(),
                    ..Default::default()
                };
                configure(&mut params);
                eng.set_parameters(params);
                let _ = client.create_preset(name);
            }
        };

        add("Flat", |_| {});
        add("Bass Boost (+6 dB)", |p| {
            p.bands[0].gain_db = 6.0;
            p.bands[1].gain_db = 3.5;
        });
        add("Électro / EDM", |p| {
            p.bands[0].gain_db = 5.5;
            p.bands[1].gain_db = 4.0;
            p.bands[3].gain_db = -2.5;
            p.bands[5].gain_db = 1.5;
            p.bands[7].gain_db = 4.5;
        });
        add("Rock / Metal", |p| {
            p.bands[0].gain_db = 2.0;
            p.bands[1].gain_db = 4.5;
            p.bands[2].gain_db = -1.5;
            p.bands[3].gain_db = -3.0;
            p.bands[5].gain_db = 3.5;
            p.bands[6].gain_db = 2.5;
            p.bands[7].gain_db = 2.0;
        });
        add("Hip-Hop / R&B", |p| {
            p.bands[0].gain_db = 7.0;
            p.bands[1].gain_db = 4.5;
            p.bands[2].gain_db = 1.0;
            p.bands[3].gain_db = -2.0;
            p.bands[5].gain_db = 1.5;
            p.bands[7].gain_db = 3.0;
        });
        add("Jazz / Warmth", |p| {
            p.bands[0].gain_db = 1.5;
            p.bands[1].gain_db = 2.5;
            p.bands[2].gain_db = 3.0;
            p.bands[3].gain_db = 1.5;
            p.bands[5].gain_db = -1.0;
            p.bands[6].gain_db = -2.0;
            p.bands[7].gain_db = -2.5;
        });
        add("Pop Moderne", |p| {
            p.bands[0].gain_db = 4.0;
            p.bands[1].gain_db = 3.0;
            p.bands[3].gain_db = -2.0;
            p.bands[4].gain_db = 1.0;
            p.bands[5].gain_db = 2.5;
            p.bands[6].gain_db = 3.0;
            p.bands[7].gain_db = 4.0;
        });
        add("Acoustic Clarity", |p| {
            p.bands[1].gain_db = -2.0;
            p.bands[4].gain_db = 2.5;
            p.bands[5].gain_db = 3.5;
            p.bands[7].gain_db = 4.0;
        });
        add("Vocal Air", |p| {
            p.bands[0].gain_db = -4.0;
            p.bands[1].gain_db = -2.5;
            p.bands[4].gain_db = 2.0;
            p.bands[5].gain_db = 3.5;
            p.bands[7].gain_db = 5.0;
        });
        add("Podcast / Voix", |p| {
            p.bands[0].gain_db = -8.0;
            p.bands[1].gain_db = -4.0;
            p.bands[2].gain_db = -2.0;
            p.bands[4].gain_db = 2.5;
            p.bands[5].gain_db = 4.0;
            p.bands[7].gain_db = 1.5;
        });
        add("Cinéma / Dialogues", |p| {
            p.bands[0].gain_db = 5.0;
            p.bands[1].gain_db = 2.0;
            p.bands[3].gain_db = -2.5;
            p.bands[4].gain_db = 2.0;
            p.bands[5].gain_db = 3.5;
            p.bands[7].gain_db = 2.0;
        });
        add("Gaming / FPS", |p| {
            p.bands[0].gain_db = -6.0;
            p.bands[1].gain_db = -3.0;
            p.bands[2].gain_db = -1.5;
            p.bands[4].gain_db = 3.0;
            p.bands[5].gain_db = 5.0;
            p.bands[6].gain_db = 4.0;
            p.bands[7].gain_db = -2.0;
        });
        add("Casque Audiophile (Harman)", |p| {
            p.bands[0].gain_db = 4.5;
            p.bands[1].gain_db = 2.5;
            p.bands[2].gain_db = -0.5;
            p.bands[5].gain_db = 2.0;
            p.bands[6].gain_db = 1.0;
            p.bands[7].gain_db = 1.5;
        });
        add("Écoute Nocturne (Night Mode)", |p| {
            p.bands[0].gain_db = -9.0;
            p.bands[1].gain_db = -5.0;
            p.bands[2].gain_db = -2.0;
            p.bands[4].gain_db = 2.0;
            p.bands[5].gain_db = 2.5;
            p.bands[6].gain_db = -2.0;
            p.bands[7].gain_db = -4.0;
        });
        add("Loudness", |p| {
            p.bands[0].gain_db = 5.0;
            p.bands[1].gain_db = 3.0;
            p.bands[3].gain_db = -2.5;
            p.bands[7].gain_db = 4.0;
        });

        // Load any user-saved presets from ~/.equaform/presets/
        let presets_dir = user_config_dir().join("presets");
        if let Ok(entries) = std::fs::read_dir(presets_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("req") {
                    if let Some(p_str) = path.to_str() {
                        let _ = client.load_preset_from_file(p_str);
                    }
                }
            }
        }

        let initial_preset = load_last_active_preset_name()
            .filter(|n| client.preset_manager.get(n).is_some())
            .unwrap_or_else(|| "Flat".to_string());
        let _ = client.apply_preset(&initial_preset);
        client._current_preset = initial_preset;

        // Restore complete saved app state (bypass, auto_gain, master gain, and exact band curves) if available
        let _ = load_complete_app_state(client);
    }

    use equaform_analysis::AnalysisEngine;
    use equaform_engine::EngineLifecycle;

    /// A real engine client that wraps the actual equaform-engine.
    struct EngineClient {
        engine: Option<Box<EngineLifecycle>>,
        analysis_engine: AnalysisEngine,
        snapshot: ParamSnapshot,
        pending_commands: GuiCommandBatch,
        preset_manager: PresetManager,
        egui_context: Option<egui::Context>,
        /// Band parameters for the frequency response plot.
        band_params: [equaform_common::BandParameters; equaform_common::MAX_BANDS],
        /// Sample rate for the frequency response plot.
        sample_rate: f32,
        /// Currently selected preset name for the combo box.
        _current_preset: String,
        /// Spectrum processor for real-time FFT visualization.
        _spectrum_processor: equaform_analysis::SpectrumProcessor,
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
                band_params: core::array::from_fn(|i| equaform_common::BandParameters {
                    frequency_hz: 20.0 * (i + 1) as f32,
                    ..Default::default()
                }),
                sample_rate: 48000.0,
                _current_preset: String::new(),
                _spectrum_processor: equaform_analysis::SpectrumProcessor::new(
                    equaform_analysis::SpectrumConfig::new(1024, 48000.0),
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
        #[allow(dead_code)]
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
                    if band_idx < equaform_common::MAX_BANDS {
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
        fn filter_type_from_f32(v: f32) -> equaform_common::FilterType {
            equaform_common::FilterType::from_raw(v.round().max(0.0) as u32)
                .unwrap_or(equaform_common::FilterType::Bell)
        }

        /// Map a float value to a Slope enum variant.
        fn slope_from_f32(v: f32) -> equaform_common::Slope {
            equaform_common::Slope::from_raw(v.round().max(0.0) as u32)
                .unwrap_or(equaform_common::Slope::Db12)
        }

        /// Map a FilterType enum to a float value (for snapshot encoding).
        fn filter_type_to_f32(ft: equaform_common::FilterType) -> f32 {
            ft.to_raw() as f32
        }

        /// Map a Slope enum to a float value (for snapshot encoding).
        fn slope_to_f32(sl: equaform_common::Slope) -> f32 {
            sl.to_raw() as f32
        }

        /// Map a ProcessingMode enum to a float value (for snapshot encoding).
        fn processing_mode_to_f32(_pm: equaform_common::ProcessingMode) -> f32 {
            0.0
        }

        /// Map a StereoMode enum to a float value (for snapshot encoding).
        fn stereo_mode_to_f32(_sm: equaform_common::StereoMode) -> f32 {
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
                values.push(ParamValue {
                    param_id: ParamId(equaform_common::global::AUTO_GAIN_COMPENSATION_DB),
                    value: eng.auto_gain_compensation_db(),
                });

                // Band parameters
                for band_idx in 0..equaform_common::MAX_BANDS {
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
        fn apply_command_to_params(params: &mut equaform_common::EqParameters, cmd: &GuiCommand) {
            let pid = cmd.param_id.0;
            if (1..=99).contains(&pid) {
                // Global parameters
                match pid {
                    1 => params.global.bypass = cmd.value > 0.5,
                    2 => params.global.output_gain_db = cmd.value.clamp(-24.0, 24.0),
                    3 => params.global.auto_gain = cmd.value > 0.5,
                    4 => {
                        params.global.processing_mode =
                            equaform_common::ProcessingMode::MinimumPhase;
                    }
                    5 => {
                        params.global.stereo_mode = equaform_common::StereoMode::Stereo;
                    }
                    _ => {}
                }
            } else if pid >= 100 {
                // Band parameters: band_idx = (pid - 100) / 100
                let band_idx = ((pid - 100) / 100) as usize;
                if band_idx >= equaform_common::MAX_BANDS {
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

            // Create a new engine safely on the heap to avoid stack overflow.
            let mut eng = std::thread::Builder::new()
                .name("engine-client-init".to_string())
                .stack_size(8 * 1024 * 1024)
                .spawn(|| Box::new(EngineLifecycle::new()))
                .expect("spawn engine thread")
                .join()
                .expect("engine init panicked");

            if eng.prepare(sample_rate as f64, block_size, channels) {
                self.engine = Some(eng);

                // Initialize band_params to default 8-band musical layout.
                self.band_params = default_8_bands();
                let mut params = *self.engine.as_ref().unwrap().eq_parameters();
                params.bands = self.band_params;
                self.engine.as_mut().unwrap().set_parameters(params);

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

            if !eng.process(&input_planar, &mut output_planar) {
                // Dry copy fallback on processing failure
                output_planar[0].copy_from_slice(input_planar[0]);
                output_planar[1].copy_from_slice(input_planar[1]);
            }

            // Update snapshot and band_params from engine before releasing borrow.
            let eq_params = *eng.eq_parameters();

            // Drain analysis frames.
            self.drain_analysis_frames();

            // Update snapshot from engine params.
            self.snapshot = ParamSnapshot::new(self.encode_snapshot());

            // Update band_params from engine.
            for i in 0..equaform_common::MAX_BANDS {
                self.band_params[i] = eq_params.bands[i];
            }
        }

        fn snapshot(&self) -> ParamSnapshot {
            self.snapshot.clone()
        }

        fn analysis_snapshot(&self) -> AnalysisSnapshot {
            let mut rms = [f32::NEG_INFINITY; equaform_common::ANALYSIS_MAX_CHANNELS];
            let mut peak = [f32::NEG_INFINITY; equaform_common::ANALYSIS_MAX_CHANNELS];
            let mut dropped = false;

            // Pull the latest analysis frame if available.
            if let Some(frame) = self.analysis_engine.latest_snapshot() {
                for (i, &v) in frame.rms.iter().enumerate() {
                    if i < equaform_common::ANALYSIS_MAX_CHANNELS {
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
                        equaform_common::FilterType::Bell => ratio * 2.0,
                        equaform_common::FilterType::LowShelf => {
                            if freq > freq_f32 as f64 {
                                ratio * 0.5
                            } else {
                                0.0
                            }
                        }
                        equaform_common::FilterType::HighShelf => {
                            if freq < freq_f32 as f64 {
                                ratio * 0.5
                            } else {
                                0.0
                            }
                        }
                        equaform_common::FilterType::LowPass => {
                            if freq > freq_f32 as f64 {
                                ratio * 6.0
                            } else {
                                0.0
                            }
                        }
                        equaform_common::FilterType::HighPass => {
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
                for i in 0..equaform_common::MAX_BANDS {
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
                // Skip read-only telemetry parameter ID 6 (auto-gain compensation readout)
                if pv.param_id.0 == 6 {
                    continue;
                }
                preset.parameter_values.set(pv.param_id.0, pv.value);
            }

            // Allow updating an existing preset with the same name.
            self.preset_manager.remove(name);
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
                let keep_bypass = params.global.bypass;
                let keep_auto_gain = params.global.auto_gain;
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
                // Preserve global Bypass and Auto Gain state when switching presets.
                params.global.bypass = keep_bypass;
                params.global.auto_gain = keep_auto_gain;
                eng.set_parameters(params);
            }

            // Update snapshot and band_params locally.
            self.snapshot = ParamSnapshot::new(self.encode_snapshot());
            let eq_params = self
                .engine
                .as_ref()
                .map(|e| *e.eq_parameters())
                .unwrap_or_default();
            for i in 0..equaform_common::MAX_BANDS {
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
                let keep_bypass = params.global.bypass;
                let keep_auto_gain = params.global.auto_gain;
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
                params.global.bypass = keep_bypass;
                params.global.auto_gain = keep_auto_gain;
                eng.set_parameters(params);
            }

            // Update snapshot and band_params locally.
            self.snapshot = ParamSnapshot::new(self.encode_snapshot());
            let eq_params = self
                .engine
                .as_ref()
                .map(|e| *e.eq_parameters())
                .unwrap_or_default();
            for i in 0..equaform_common::MAX_BANDS {
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
            if band_index < equaform_common::MAX_BANDS {
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
