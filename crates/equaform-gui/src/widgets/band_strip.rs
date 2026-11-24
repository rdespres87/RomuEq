//! Single-band EQ parameter strip widget.

use crate::i18n::Translations;
use crate::{BandParams, GuiCommandBatch, ParamSnapshot};

// ---------------------------------------------------------------------------
// Band widget rendering (egui integration)
// ---------------------------------------------------------------------------

/// Immutable per-band rendering context for [`render_band`].
#[derive(Clone, Copy)]
pub struct BandRenderContext<'a> {
    pub band_index: usize,
    pub params: &'a BandParams,
    pub snapshot: &'a ParamSnapshot,
    pub translations: &'static Translations,
}

/// Render a single band's controls.
///
/// Each control maps to exactly one stable ParamId and validates/clamps values
/// before emitting commands through the batch. Only pushes a command when the
/// value actually changes (tracked via egui response.changed()).
pub fn render_band(
    ui: &mut egui::Ui,
    ctx: BandRenderContext<'_>,
    batch: &mut GuiCommandBatch,
) {
    let band_index = ctx.band_index;
    let params = ctx.params;
    let snapshot = ctx.snapshot;
    let t = ctx.translations;
    let band_name = t
        .band_names
        .get(band_index)
        .copied()
        .unwrap_or(t.band_fallback);

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
            let filter_types = t.filter_types;
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
                    .on_hover_text(t.tooltip_reset_gain)
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
                .on_hover_text(t.tooltip_center_freq)
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
                .on_hover_text(t.tooltip_q_factor)
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

