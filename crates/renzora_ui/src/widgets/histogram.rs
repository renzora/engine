//! Histogram bar-chart widget.
//!
//! Reusable for image channel histograms, audio spectra, profiler samples.
//! Accepts a pre-binned `&[f32]`; normalization happens inside.

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Configuration for the histogram widget.
#[derive(Clone, Copy, Debug)]
pub struct HistogramConfig {
    pub height: f32,
    /// Max bin value to normalize against. `None` = auto (use data max).
    pub y_max: Option<f32>,
    /// Show min/max tick labels.
    pub show_labels: bool,
}

impl Default for HistogramConfig {
    fn default() -> Self {
        Self { height: 80.0, y_max: None, show_labels: false }
    }
}

/// Render a vertical-bar histogram. Returns the allocated response.
pub fn histogram(
    ui: &mut egui::Ui,
    bins: &[f32],
    cfg: HistogramConfig,
    color: Color32,
    theme: &Theme,
) -> egui::Response {
    let w = ui.available_width().max(60.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, cfg.height), Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 2.0, theme.surfaces.faint.to_color32());
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );

    if bins.is_empty() {
        return response;
    }
    let y_max = cfg
        .y_max
        .unwrap_or_else(|| bins.iter().cloned().fold(0.0_f32, f32::max))
        .max(1e-6);
    let bar_w = rect.width() / bins.len() as f32;
    for (i, v) in bins.iter().enumerate() {
        let h = rect.height() * (v / y_max).clamp(0.0, 1.0);
        let x = rect.min.x + i as f32 * bar_w;
        painter.rect_filled(
            egui::Rect::from_min_max(
                Pos2::new(x, rect.max.y - h),
                Pos2::new(x + bar_w - 0.5, rect.max.y),
            ),
            0.0,
            color,
        );
    }

    if cfg.show_labels {
        let muted = theme.text.muted.to_color32();
        painter.text(
            Pos2::new(rect.min.x + 2.0, rect.min.y + 2.0),
            egui::Align2::LEFT_TOP,
            format!("{:.2}", y_max),
            egui::FontId::proportional(9.0),
            muted,
        );
    }
    response
}
