//! Frequency response plot widgets (static and interactive node-dragging).

use crate::i18n::{Language, Translations};
use crate::widgets::{log_freq_axis_formatter, log_freq_grid_spacer};
use crate::{BandParams, GuiCommandBatch};
use egui_plot::{HLine, Line, Plot, Points, Text, VLine};
use equaform_common::{BandParameters, FilterType, MAX_BANDS};
use equaform_dsp::coefficients::FilterSpecification;
use equaform_dsp::{biquad_response_at_frequency, compute_response};

/// Immutable rendering parameters for frequency response plots.
#[derive(Clone, Copy, Debug)]
pub struct PlotContext<'a> {
    pub bands: &'a [BandParameters; MAX_BANDS],
    pub sample_rate: f32,
    pub width: usize,
    pub translations: &'static Translations,
}

const LOG_MIN_X: f64 = 1.204_119_982_655_924_8; // 16.0_f64.log10()
const LOG_MAX_X: f64 = 4.380_211_241_711_606; // 24000.0_f64.log10()

fn band_color(index: usize) -> egui::Color32 {
    match index % 8 {
        0 => egui::Color32::from_rgb(255, 80, 80),   // Red
        1 => egui::Color32::from_rgb(255, 160, 40),  // Orange
        2 => egui::Color32::from_rgb(255, 220, 50),  // Yellow
        3 => egui::Color32::from_rgb(50, 220, 100),  // Green
        4 => egui::Color32::from_rgb(40, 200, 220),  // Cyan
        5 => egui::Color32::from_rgb(80, 140, 255),  // Blue
        6 => egui::Color32::from_rgb(180, 90, 255),  // Purple
        _ => egui::Color32::from_rgb(255, 100, 200), // Magenta
    }
}

fn band_node_coords(band: &BandParameters) -> (f64, f64) {
    let node_x = (band.frequency_hz as f64).clamp(20.0, 20_000.0).log10();
    let node_y = match band.filter_type {
        FilterType::LowPass | FilterType::HighPass => 0.0,
        _ => (band.gain_db as f64).clamp(-24.0, 24.0),
    };
    (node_x, node_y)
}

fn find_closest_band_node(
    bands: &[BandParameters; MAX_BANDS],
    pointer_xy: (f64, f64),
    plot_size: (f64, f64),
    max_dist_px_sq: f64,
) -> Option<usize> {
    let (px, py) = pointer_xy;
    let (plot_w, plot_h) = plot_size;
    let mut closest = None;
    let mut min_dist_px = f64::MAX;

    for (i, band) in bands.iter().enumerate() {
        if !band.enabled {
            continue;
        }
        let (node_x, node_y) = band_node_coords(band);
        let dx_px = ((px - node_x) / (LOG_MAX_X - LOG_MIN_X)) * plot_w;
        let dy_px = ((py - node_y) / 80.0) * plot_h;
        let dist_px_sq = dx_px * dx_px + dy_px * dy_px;
        if dist_px_sq < max_dist_px_sq && dist_px_sq < min_dist_px {
            min_dist_px = dist_px_sq;
            closest = Some(i);
        }
    }

    closest
}

fn draw_reference_lines(plot_ui: &mut egui_plot::PlotUi) {
    let zero_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40);
    plot_ui.hline(
        HLine::new(0.0)
            .color(zero_color)
            .stroke(egui::Stroke::new(1.0, zero_color)),
    );
    let limit_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 16);
    for limit_y in [-24.0, 24.0] {
        plot_ui.hline(
            HLine::new(limit_y)
                .color(limit_color)
                .stroke(egui::Stroke::new(0.8, limit_color)),
        );
    }
}

fn draw_individual_band_curves(
    plot_ui: &mut egui_plot::PlotUi,
    plot_ctx: PlotContext<'_>,
) {
    let log_min = 20.0_f32.log10();
    let log_max = 20_000.0_f32.log10();
    let denom = (plot_ctx.width - 1).max(1) as f32;

    for (i, band) in plot_ctx.bands.iter().enumerate() {
        let is_active_curve = band.enabled
            && (band.gain_db.abs() > 0.05
                || matches!(band.filter_type, FilterType::LowPass | FilterType::HighPass));
        if !is_active_curve {
            continue;
        }

        let color = band_color(i);
        let coeffs = FilterSpecification::from_band(band, plot_ctx.sample_rate).compute();
        let band_curve_points: Vec<[f64; 2]> = (0..plot_ctx.width)
            .map(|k| {
                let t = k as f32 / denom;
                let f = 10.0_f32.powf(log_min + t * (log_max - log_min));
                let db = biquad_response_at_frequency(
                    &coeffs,
                    f.min(plot_ctx.sample_rate * 0.4999),
                    plot_ctx.sample_rate,
                );
                [
                    (f as f64).clamp(20.0, 20_000.0).log10(),
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

fn draw_band_nodes(
    plot_ui: &mut egui_plot::PlotUi,
    bands: &[BandParameters; MAX_BANDS],
    active_dragged: Option<usize>,
    t: &Translations,
) {
    for (i, band) in bands.iter().enumerate() {
        if !band.enabled {
            continue;
        }

        let color = band_color(i);
        let (node_x, node_y) = band_node_coords(band);

        let guide_color =
            egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 32);
        plot_ui.vline(
            VLine::new(node_x)
                .color(guide_color)
                .stroke(egui::Stroke::new(1.0, guide_color)),
        );

        let is_active_node = active_dragged == Some(i);
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
                .name(format!("{} {}", t.band_prefix, i + 1)),
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

/// Renders the frequency response plot for the EQ bands (English default).
///
/// Computes the combined magnitude response across log-spaced frequencies
/// from 20 Hz to 20 kHz and draws it as a smooth curve in an `egui_plot::Plot`.
pub fn render_frequency_response_plot(
    ui: &mut egui::Ui,
    bands: &[BandParameters; MAX_BANDS],
    sample_rate: f32,
    width: usize,
) {
    render_frequency_response_plot_localized(
        ui,
        PlotContext {
            bands,
            sample_rate,
            width,
            translations: Translations::for_language(Language::English),
        },
    );
}

/// Renders the frequency response plot using the provided translation catalog.
pub fn render_frequency_response_plot_localized(ui: &mut egui::Ui, plot_ctx: PlotContext<'_>) {
    let bands = plot_ctx.bands;
    let t = plot_ctx.translations;
    let response = compute_response(plot_ctx.sample_rate, bands, plot_ctx.width);

    let points: Vec<[f64; 2]> = response
        .iter()
        .map(|p| {
            [
                (p.frequency_hz as f64).clamp(20.0, 20_000.0).log10(),
                (p.magnitude_db as f64).clamp(-40.0, 40.0),
            ]
        })
        .collect();

    let line = Line::new(points)
        .color(egui::Color32::from_rgb(0, 180, 255))
        .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 180, 255)))
        .name(t.eq_response_series);

    let freq_short = t.tooltip_freq_short;
    let gain_short = t.tooltip_gain_short;
    let plot = Plot::new("eq_response")
        .include_x(LOG_MIN_X)
        .include_x(LOG_MAX_X)
        .include_y(-40.0)
        .include_y(40.0)
        .height(200.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .x_axis_formatter(log_freq_axis_formatter)
        .x_grid_spacer(log_freq_grid_spacer)
        .label_formatter(move |_name, val| {
            let freq = 10.0_f64.powf(val.x);
            format!("{freq_short}: {freq:.0} Hz\n{gain_short}: {:+.1} dB", val.y)
        })
        .show(ui, |plot_ui| {
            plot_ui.set_auto_bounds(egui::Vec2b::FALSE);
            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                [LOG_MIN_X, -40.0],
                [LOG_MAX_X, 40.0],
            ));
            plot_ui.hline(HLine::new(0.0).color(egui::Color32::GRAY));
            plot_ui.line(line);

            for (i, band) in bands.iter().enumerate() {
                if !band.enabled {
                    continue;
                }
                let color = band_color(i);
                let (node_x, node_y) = band_node_coords(band);
                plot_ui.points(
                    Points::new(vec![[node_x, node_y]])
                        .color(color)
                        .radius(5.0)
                        .name(format!("{} {}", t.band_prefix, i + 1)),
                );
                plot_ui.text(
                    Text::new(
                        egui_plot::PlotPoint::new(node_x, (node_y + 2.5).min(36.0)),
                        format!("B{}", i + 1),
                    )
                    .color(color),
                );
            }
        });

    ui.horizontal(|ui| {
        ui.label(t.axis_freq);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(t.axis_gain);
        });
    });

    let _ = plot;
}

/// Renders the interactive frequency response plot allowing direct mouse node dragging.
pub fn render_interactive_frequency_response_plot(
    ui: &mut egui::Ui,
    plot_ctx: PlotContext<'_>,
    dragged_band: &mut Option<usize>,
    batch: &mut GuiCommandBatch,
) {
    let bands = plot_ctx.bands;
    let t = plot_ctx.translations;
    let response = compute_response(plot_ctx.sample_rate, bands, plot_ctx.width);

    let points: Vec<[f64; 2]> = response
        .iter()
        .map(|p| {
            [
                (p.frequency_hz as f64).clamp(20.0, 20_000.0).log10(),
                (p.magnitude_db as f64).clamp(-40.0, 40.0),
            ]
        })
        .collect();

    let master_line = Line::new(points)
        .color(egui::Color32::from_rgb(0, 185, 255))
        .stroke(egui::Stroke::new(2.2, egui::Color32::from_rgb(0, 185, 255)))
        .fill(0.0)
        .name(t.eq_curve_series);

    let is_primary_down = ui.ctx().input(|i| i.pointer.primary_down());
    let is_primary_pressed = ui.ctx().input(|i| i.pointer.primary_pressed());
    let mut local_dragged = *dragged_band;
    let mut coord_under_pointer = None;

    let freq_short = t.tooltip_freq_short;
    let gain_short = t.tooltip_gain_short;
    let plot = Plot::new("eq_response_interactive")
        .include_x(LOG_MIN_X)
        .include_x(LOG_MAX_X)
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
        .label_formatter(move |_name, val| {
            let freq = 10.0_f64.powf(val.x);
            format!("{freq_short}: {freq:.0} Hz\n{gain_short}: {:+.1} dB", val.y)
        })
        .show(ui, |plot_ui| {
            plot_ui.set_auto_bounds(egui::Vec2b::FALSE);
            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                [LOG_MIN_X, -40.0],
                [LOG_MAX_X, 40.0],
            ));

            draw_reference_lines(plot_ui);
            draw_individual_band_curves(plot_ui, plot_ctx);
            plot_ui.line(master_line);

            if let Some(coord) = plot_ui.pointer_coordinate() {
                coord_under_pointer = Some((coord.x, coord.y));
            }

            draw_band_nodes(plot_ui, bands, local_dragged, t);
        });

    let plot_hovered = plot.response.hovered();
    let plot_rect = plot.response.rect;
    let plot_size = (
        plot_rect.width().max(100.0) as f64,
        plot_rect.height().max(50.0) as f64,
    );

    if is_primary_down {
        if let Some((px, py)) = coord_under_pointer {
            if local_dragged.is_none() && is_primary_pressed && plot_hovered {
                local_dragged = find_closest_band_node(bands, (px, py), plot_size, 256.0);
            }

            if let Some(band_idx) = local_dragged {
                let params = BandParams::new(band_idx, (band_idx as u32 + 1) * 100);
                let new_freq =
                    (10.0_f64.powf(px.clamp(LOG_MIN_X, LOG_MAX_X)) as f32).clamp(20.0, 20_000.0);
                let new_gain = (py as f32).clamp(-24.0, 24.0);
                batch.push(params.frequency_hz(), new_freq);
                match bands[band_idx].filter_type {
                    FilterType::LowPass | FilterType::HighPass => {}
                    _ => batch.push(params.gain_db(), new_gain),
                }
            }
        }
    } else {
        local_dragged = None;
    }

    let scroll_delta = ui.ctx().input(|i| i.raw_scroll_delta.y);
    if scroll_delta.abs() > 0.1 && (plot_hovered || local_dragged.is_some()) {
        let target_band = local_dragged.or_else(|| {
            coord_under_pointer
                .and_then(|pt| find_closest_band_node(bands, pt, plot_size, 400.0))
        });

        if let Some(band_idx) = target_band {
            let params = BandParams::new(band_idx, (band_idx as u32 + 1) * 100);
            let factor = if scroll_delta > 0.0 { 1.10 } else { 0.90 };
            let new_q = (bands[band_idx].q * factor).clamp(0.1, 30.0);
            batch.push(params.q(), new_q);
        }
    }

    *dragged_band = local_dragged;

    let _ = plot;
}
