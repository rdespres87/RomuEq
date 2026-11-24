//! Frequency response plot widgets (static and interactive node-dragging).

use crate::i18n::{Language, Translations};
use crate::widgets::{log_freq_axis_formatter, log_freq_grid_spacer};
use crate::{BandParams, GuiCommandBatch};
use egui_plot::{HLine, Line, Plot, Points, Text, VLine};
use equaform_common::{BandParameters, FilterType, MAX_BANDS, Slope};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BandCacheKey {
    enabled: bool,
    filter_type: FilterType,
    frequency_hz_bits: u32,
    gain_db_bits: u32,
    q_bits: u32,
    slope: Slope,
}

impl BandCacheKey {
    fn from_band(band: &BandParameters) -> Self {
        Self {
            enabled: band.enabled,
            filter_type: band.filter_type,
            frequency_hz_bits: band.frequency_hz.to_bits(),
            gain_db_bits: band.gain_db.to_bits(),
            q_bits: band.q.to_bits(),
            slope: band.slope,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CurveCacheKey {
    bands: [BandCacheKey; MAX_BANDS],
    sample_rate_bits: u32,
    width: usize,
}

impl CurveCacheKey {
    fn new(bands: &[BandParameters; MAX_BANDS], sample_rate: f32, width: usize) -> Self {
        Self {
            bands: core::array::from_fn(|i| BandCacheKey::from_band(&bands[i])),
            sample_rate_bits: sample_rate.to_bits(),
            width,
        }
    }
}

/// Dirty-flag cache for the master EQ response curve and per-band curves.
///
/// Avoids recomputing biquad frequency response DSP math on every 60 FPS UI frame
/// when EQ parameters, sample rate, and plot width are unchanged.
#[derive(Debug, Default)]
pub struct EqCurveCache {
    last_key: Option<CurveCacheKey>,
    master_points: Vec<[f64; 2]>,
    band_curves: [Option<Vec<[f64; 2]>>; MAX_BANDS],
    recompute_count: u64,
}

impl EqCurveCache {
    /// Creates an empty curve cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Recomputes cached master and per-band curves only when inputs differ from the last key.
    pub fn ensure_updated(
        &mut self,
        bands: &[BandParameters; MAX_BANDS],
        sample_rate: f32,
        width: usize,
    ) {
        let key = CurveCacheKey::new(bands, sample_rate, width);
        if self.last_key == Some(key) {
            return;
        }

        let prev_key = self.last_key;
        let same_grid = prev_key
            .map(|pk| pk.sample_rate_bits == key.sample_rate_bits && pk.width == key.width)
            .unwrap_or(false);

        let response = compute_response(sample_rate, bands, width);
        self.master_points.clear();
        self.master_points.extend(response.iter().map(|p| {
            [
                (p.frequency_hz as f64).clamp(20.0, 20_000.0).log10(),
                (p.magnitude_db as f64).clamp(-40.0, 40.0),
            ]
        }));

        let log_min = 20.0_f32.log10();
        let log_max = 20_000.0_f32.log10();
        let denom = width.saturating_sub(1).max(1) as f32;
        let nyquist_limit = sample_rate * 0.4999;

        for (i, band) in bands.iter().enumerate() {
            let band_unchanged = same_grid
                && prev_key
                    .map(|pk| pk.bands[i] == key.bands[i])
                    .unwrap_or(false);
            if band_unchanged {
                continue;
            }

            let is_active_curve = band.enabled
                && (band.gain_db.abs() > 0.05
                    || matches!(band.filter_type, FilterType::LowPass | FilterType::HighPass));
            if !is_active_curve {
                self.band_curves[i] = None;
                continue;
            }

            let coeffs = FilterSpecification::from_band(band, sample_rate).compute();
            let points_iter = (0..width).map(|k| {
                let t = k as f32 / denom;
                let f = 10.0_f32.powf(log_min + t * (log_max - log_min));
                let db = biquad_response_at_frequency(&coeffs, f.min(nyquist_limit), sample_rate);
                [
                    (f as f64).clamp(20.0, 20_000.0).log10(),
                    (db as f64).clamp(-40.0, 40.0),
                ]
            });

            let slot = self.band_curves[i].get_or_insert_with(|| Vec::with_capacity(width));
            slot.clear();
            slot.extend(points_iter);
        }

        self.last_key = Some(key);
        self.recompute_count = self.recompute_count.saturating_add(1);
    }

    /// Returns the cached master frequency response points `(log10(freq_hz), magnitude_db)`.
    pub fn master_points(&self) -> &[[f64; 2]] {
        &self.master_points
    }

    /// Returns the cached curve for an individual active band, or `None` if inactive.
    pub fn band_curve(&self, band_index: usize) -> Option<&[[f64; 2]]> {
        self.band_curves
            .get(band_index)
            .and_then(|opt| opt.as_deref())
    }

    /// Returns the number of cache recomputations performed since creation.
    pub fn recompute_count(&self) -> u64 {
        self.recompute_count
    }

    /// Marks the cache dirty so the next `ensure_updated` call forces a recomputation.
    pub fn invalidate(&mut self) {
        self.last_key = None;
    }
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

fn draw_individual_band_curves(plot_ui: &mut egui_plot::PlotUi, cache: &EqCurveCache) {
    for i in 0..MAX_BANDS {
        let Some(band_curve_points) = cache.band_curve(i) else {
            continue;
        };

        let color = band_color(i);
        let translucent_color =
            egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 85);
        plot_ui.line(
            Line::new(band_curve_points.to_vec())
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
///
/// Uses `curve_cache` to avoid recomputing master and per-band frequency response curves
/// when EQ parameters, sample rate, and plot width are unchanged across 60 FPS frames.
pub fn render_interactive_frequency_response_plot(
    ui: &mut egui::Ui,
    plot_ctx: PlotContext<'_>,
    curve_cache: &mut EqCurveCache,
    dragged_band: &mut Option<usize>,
    batch: &mut GuiCommandBatch,
) {
    let bands = plot_ctx.bands;
    let t = plot_ctx.translations;
    curve_cache.ensure_updated(bands, plot_ctx.sample_rate, plot_ctx.width);

    let master_line = Line::new(curve_cache.master_points().to_vec())
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
            draw_individual_band_curves(plot_ui, curve_cache);
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
            coord_under_pointer.and_then(|pt| find_closest_band_node(bands, pt, plot_size, 400.0))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bands() -> [BandParameters; MAX_BANDS] {
        let mut bands = [BandParameters::default(); MAX_BANDS];
        bands[0] = BandParameters {
            enabled: true,
            filter_type: FilterType::Bell,
            frequency_hz: 1_000.0,
            gain_db: 6.0,
            q: 1.5,
            slope: Slope::Db12,
        };
        bands[1] = BandParameters {
            enabled: true,
            filter_type: FilterType::LowShelf,
            frequency_hz: 120.0,
            gain_db: 0.0,
            q: 0.707,
            slope: Slope::Db12,
        };
        bands[2] = BandParameters {
            enabled: false,
            filter_type: FilterType::HighPass,
            frequency_hz: 80.0,
            gain_db: 0.0,
            q: 0.707,
            slope: Slope::Db24,
        };
        bands
    }

    #[test]
    fn test_curve_cache_hit_skips_recompute() {
        let bands = sample_bands();
        let mut cache = EqCurveCache::new();

        cache.ensure_updated(&bands, 48_000.0, 128);
        assert_eq!(cache.recompute_count(), 1);
        assert_eq!(cache.master_points().len(), 128);

        // Identical parameters must hit cache without recomputation.
        cache.ensure_updated(&bands, 48_000.0, 128);
        cache.ensure_updated(&bands, 48_000.0, 128);
        assert_eq!(cache.recompute_count(), 1);
    }

    #[test]
    fn test_curve_cache_invalidates_on_band_param_change() {
        let mut bands = sample_bands();
        let mut cache = EqCurveCache::new();

        cache.ensure_updated(&bands, 48_000.0, 64);
        assert_eq!(cache.recompute_count(), 1);

        bands[0].gain_db = 3.0;
        cache.ensure_updated(&bands, 48_000.0, 64);
        assert_eq!(cache.recompute_count(), 2);

        bands[0].frequency_hz = 2_000.0;
        cache.ensure_updated(&bands, 48_000.0, 64);
        assert_eq!(cache.recompute_count(), 3);

        bands[0].q = 2.5;
        cache.ensure_updated(&bands, 48_000.0, 64);
        assert_eq!(cache.recompute_count(), 4);

        bands[0].enabled = false;
        cache.ensure_updated(&bands, 48_000.0, 64);
        assert_eq!(cache.recompute_count(), 5);

        bands[2].enabled = true;
        bands[2].filter_type = FilterType::LowPass;
        cache.ensure_updated(&bands, 48_000.0, 64);
        assert_eq!(cache.recompute_count(), 6);
    }

    #[test]
    fn test_curve_cache_invalidates_on_sample_rate_or_width_change() {
        let bands = sample_bands();
        let mut cache = EqCurveCache::new();

        cache.ensure_updated(&bands, 44_100.0, 64);
        assert_eq!(cache.recompute_count(), 1);
        assert_eq!(cache.master_points().len(), 64);

        cache.ensure_updated(&bands, 48_000.0, 64);
        assert_eq!(cache.recompute_count(), 2);

        cache.ensure_updated(&bands, 48_000.0, 256);
        assert_eq!(cache.recompute_count(), 3);
        assert_eq!(cache.master_points().len(), 256);
    }

    #[test]
    fn test_curve_cache_matches_direct_compute_response() {
        let bands = sample_bands();
        let mut cache = EqCurveCache::new();
        cache.ensure_updated(&bands, 48_000.0, 128);

        let direct = compute_response(48_000.0, &bands, 128);
        assert_eq!(cache.master_points().len(), direct.len());

        for (cached, direct_pt) in cache.master_points().iter().zip(direct.iter()) {
            let expected_x = (direct_pt.frequency_hz as f64)
                .clamp(20.0, 20_000.0)
                .log10();
            let expected_y = (direct_pt.magnitude_db as f64).clamp(-40.0, 40.0);
            assert!((cached[0] - expected_x).abs() < 1e-9);
            assert!((cached[1] - expected_y).abs() < 1e-9);
        }
    }

    #[test]
    fn test_curve_cache_band_curves_active_and_inactive() {
        let bands = sample_bands();
        let mut cache = EqCurveCache::new();
        cache.ensure_updated(&bands, 48_000.0, 64);

        // Band 0 is enabled with +6 dB gain -> active curve of length 64.
        assert_eq!(cache.band_curve(0).map(|pts| pts.len()), Some(64));
        // Band 1 is enabled with 0 dB gain (non-LP/HP) -> inactive.
        assert!(cache.band_curve(1).is_none());
        // Band 2 is disabled -> inactive.
        assert!(cache.band_curve(2).is_none());
        // Out-of-bounds band index safely returns None.
        assert!(cache.band_curve(MAX_BANDS + 1).is_none());

        cache.invalidate();
        cache.ensure_updated(&bands, 48_000.0, 64);
        assert_eq!(cache.recompute_count(), 2);
    }
}
