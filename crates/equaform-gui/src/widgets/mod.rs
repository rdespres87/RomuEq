//! Modular egui widgets for the Equaform equalizer UI.

pub mod band_strip;
pub mod eq_plot;
pub mod master_section;
pub mod spectrum_view;
pub mod vu_meter;

pub use band_strip::{BandRenderContext, render_band};
pub use eq_plot::{
    EqCurveCache, PlotContext, render_frequency_response_plot,
    render_frequency_response_plot_localized, render_interactive_frequency_response_plot,
};
pub use master_section::{render_master_section, render_master_section_localized};
pub use spectrum_view::{render_spectrum_analyzer, render_spectrum_analyzer_localized};
pub use vu_meter::{render_stereo_vu_meter, render_vertical_stereo_vu_meter};

use egui_plot::GridMark;

pub(crate) fn log_freq_grid_spacer(_input: egui_plot::GridInput) -> Vec<GridMark> {
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

pub(crate) fn log_freq_axis_formatter(
    mark: GridMark,
    _range: &std::ops::RangeInclusive<f64>,
) -> String {
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
