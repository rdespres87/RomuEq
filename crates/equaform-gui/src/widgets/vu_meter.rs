//! Stereo peak VU meter widgets (horizontal and vertical).

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

/// Layout geometry for the vertical stereo VU-meter.
#[derive(Clone, Copy)]
struct VerticalVuLayout {
    rect: egui::Rect,
    bar_w: f32,
    gap: f32,
    meter_top: f32,
    meter_bottom: f32,
    meter_h: f32,
}

impl VerticalVuLayout {
    fn db_to_norm(db: f32) -> f32 {
        ((db.clamp(-60.0, 6.0) + 60.0) / 66.0).clamp(0.0, 1.0)
    }

    fn db_to_y(self, db: f32) -> f32 {
        self.meter_bottom - Self::db_to_norm(db) * self.meter_h
    }
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

    let meter_top = rect.min.y + 14.0;
    let meter_bottom = rect.max.y - 4.0;
    let layout = VerticalVuLayout {
        rect,
        bar_w,
        gap,
        meter_top,
        meter_bottom,
        meter_h: (meter_bottom - meter_top).max(20.0),
    };
    let painter = ui.painter();
    draw_vertical_vu_scale(painter, layout);
    draw_vertical_vu_channel(painter, layout, rect.min.x, peak_l_db);
    draw_vertical_vu_channel(painter, layout, rect.min.x + bar_w + gap, peak_r_db);
}

fn draw_vertical_vu_scale(painter: &egui::Painter, layout: VerticalVuLayout) {
    const DB_MARKS: [(f32, &str); 7] = [
        (6.0, "+6"),
        (0.0, "0"),
        (-6.0, "-6"),
        (-12.0, "-12"),
        (-24.0, "-24"),
        (-40.0, "-40"),
        (-60.0, "-60"),
    ];
    let l_center_x = layout.rect.min.x + layout.bar_w * 0.5;
    let r_center_x = layout.rect.min.x + layout.bar_w + layout.gap + layout.bar_w * 0.5;
    for (x, label) in [(l_center_x, "L"), (r_center_x, "R")] {
        painter.text(
            egui::pos2(x, layout.rect.min.y + 5.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(9.0),
            egui::Color32::from_rgb(170, 180, 195),
        );
    }

    let ticks_x = layout.rect.min.x + layout.bar_w * 2.0 + layout.gap;
    for &(db, text) in &DB_MARKS {
        let y = layout.db_to_y(db);
        let color = if db > 0.0 {
            egui::Color32::from_rgb(250, 90, 90)
        } else if db == 0.0 {
            egui::Color32::from_rgb(240, 240, 240)
        } else {
            egui::Color32::from_rgb(120, 130, 145)
        };
        painter.text(
            egui::pos2(ticks_x + 4.0, y),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::monospace(8.0),
            color,
        );
        painter.line_segment(
            [egui::pos2(ticks_x, y), egui::pos2(ticks_x + 2.0, y)],
            egui::Stroke::new(0.6, egui::Color32::from_rgb(60, 65, 75)),
        );
    }
}

fn draw_vertical_vu_channel(
    painter: &egui::Painter,
    layout: VerticalVuLayout,
    x_min: f32,
    db: f32,
) {
    let channel_rect = egui::Rect::from_min_max(
        egui::pos2(x_min, layout.meter_top),
        egui::pos2(x_min + layout.bar_w, layout.meter_bottom),
    );
    painter.rect_filled(channel_rect, 1.5, egui::Color32::from_rgb(24, 26, 32));

    let norm = VerticalVuLayout::db_to_norm(db);
    if norm > 0.005 {
        let green_top = layout.meter_bottom - norm.min(48.0 / 66.0) * layout.meter_h;
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(x_min, green_top),
                egui::pos2(x_min + layout.bar_w, layout.meter_bottom),
            ),
            1.5,
            egui::Color32::from_rgb(45, 215, 115),
        );
        if norm > 48.0 / 66.0 {
            let amber_top = layout.meter_bottom - norm.min(57.0 / 66.0) * layout.meter_h;
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x_min, amber_top),
                    egui::pos2(
                        x_min + layout.bar_w,
                        layout.meter_bottom - (48.0 / 66.0) * layout.meter_h,
                    ),
                ),
                1.0,
                egui::Color32::from_rgb(245, 195, 50),
            );
        }
        if norm > 57.0 / 66.0 {
            let fill_top = layout.meter_bottom - norm * layout.meter_h;
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x_min, fill_top),
                    egui::pos2(
                        x_min + layout.bar_w,
                        layout.meter_bottom - (57.0 / 66.0) * layout.meter_h,
                    ),
                ),
                1.0,
                egui::Color32::from_rgb(250, 70, 70),
            );
        }
    }

    if db >= -0.1 {
        let clip_rect = egui::Rect::from_min_max(
            egui::pos2(x_min, layout.meter_top - 3.0),
            egui::pos2(x_min + layout.bar_w, layout.meter_top),
        );
        painter.rect_filled(clip_rect, 1.0, egui::Color32::from_rgb(255, 40, 40));
    }

    painter.rect_stroke(
        channel_rect,
        1.5,
        egui::Stroke::new(0.7, egui::Color32::from_rgb(55, 60, 70)),
        egui::StrokeKind::Inside,
    );
}
