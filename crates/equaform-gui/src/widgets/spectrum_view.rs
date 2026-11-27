//! Real-time FFT spectrum analyzer widget.

use crate::AnalysisSnapshot;
use crate::i18n::{Language, Translations};
use crate::widgets::{log_freq_axis_formatter, log_freq_grid_spacer};
use egui_plot::{Line, Plot};

// ---------------------------------------------------------------------------
// Spectrum analyzer widget
// ---------------------------------------------------------------------------

/// Renders a real-time spectrum analyzer from analysis data (English default).
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
    render_spectrum_analyzer_localized(
        ui,
        analysis,
        sample_rate,
        num_bars,
        Translations::for_language(Language::English),
    );
}

/// Renders a real-time spectrum analyzer using the provided translation catalog.
pub fn render_spectrum_analyzer_localized(
    ui: &mut egui::Ui,
    analysis: &AnalysisSnapshot,
    sample_rate: f32,
    num_bars: usize,
    t: &'static Translations,
) {
    let log_min = 16.0_f64.log10();
    let log_max = (f64::from(sample_rate * 0.5)).min(24_000.0).log10();
    let bars = build_spectrum_bars(&analysis.spectrum_data, num_bars, log_min, log_max);

    let bar_chart = Line::new(bars)
        .color(egui::Color32::from_rgb(0, 230, 180))
        .stroke(egui::Stroke::new(1.8, egui::Color32::from_rgb(0, 230, 180)))
        .fill(-90.0)
        .name(t.spectrum_series);

    let freq_short = t.tooltip_freq_short;
    let level_short = t.tooltip_level_short;
    let _ = Plot::new("spectrum")
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
        .label_formatter(move |_name, val| {
            let freq = 10.0_f64.powf(val.x);
            format!(
                "{freq_short}: {freq:.0} Hz\n{level_short}: {:+.1} dBFS",
                val.y
            )
        })
        .show(ui, |plot_ui| {
            plot_ui.set_auto_bounds(egui::Vec2b::FALSE);
            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                [log_min, -90.0],
                [log_max, 0.0],
            ));
            plot_ui.line(bar_chart);
        });
}

fn build_spectrum_bars(
    spectrum: &[[f64; 2]],
    num_bars: usize,
    log_min: f64,
    log_max: f64,
) -> Vec<[f64; 2]> {
    let mut bars = Vec::with_capacity(num_bars);
    for i in 0..num_bars {
        let ratio = if num_bars == 1 {
            0.5_f64
        } else {
            i as f64 / (num_bars - 1) as f64
        };
        let log_freq = log_min + ratio * (log_max - log_min);
        let mag_db = if spectrum.is_empty() {
            -90.0
        } else {
            interpolate_spectrum_db(spectrum, 10.0_f64.powf(log_freq)).clamp(-90.0, 0.0)
        };
        bars.push([log_freq, mag_db]);
    }
    bars
}

fn interpolate_spectrum_db(spectrum: &[[f64; 2]], target_freq: f64) -> f64 {
    let (lower_idx, upper_idx) = if spectrum.len() > 1 {
        let idx = spectrum.partition_point(|pt| pt[0] < target_freq);
        if idx == 0 {
            (0, 1)
        } else if idx >= spectrum.len() {
            (spectrum.len() - 1, spectrum.len() - 1)
        } else {
            (idx - 1, idx)
        }
    } else {
        (0, 0)
    };

    let lo = spectrum[lower_idx];
    let hi = spectrum[upper_idx];
    if lo[0] == hi[0] {
        lo[1]
    } else {
        let frac = (target_freq - lo[0]) / (hi[0] - lo[0]);
        lo[1] + frac * (hi[1] - lo[1])
    }
}
