//! Real-time FFT spectrum analyzer widget.

use crate::AnalysisSnapshot;
use crate::widgets::{log_freq_axis_formatter, log_freq_grid_spacer};
use egui_plot::{Line, Plot};

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

