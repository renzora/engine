//! Audio waveform renderer.
//!
//! Renders a mono or peak-pair waveform across a horizontal rect. Input is
//! pre-summarized samples (min/max per bucket) for efficient large-audio
//! display. Peaks are drawn as a filled envelope.

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Configuration for `waveform`.
#[derive(Clone, Copy, Debug)]
pub struct WaveformConfig {
    pub height: f32,
    pub show_center_line: bool,
}

impl Default for WaveformConfig {
    fn default() -> Self {
        Self { height: 60.0, show_center_line: true }
    }
}

/// Render a waveform from `peaks`, where each entry is `(min, max)` in
/// `-1.0..1.0`. Buckets are drawn left-to-right across the widget.
pub fn waveform(
    ui: &mut egui::Ui,
    peaks: &[(f32, f32)],
    cfg: WaveformConfig,
    color: Color32,
    theme: &Theme,
) -> egui::Response {
    let w = ui.available_width().max(60.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, cfg.height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, theme.surfaces.faint.to_color32());

    if cfg.show_center_line {
        let muted = theme.widgets.border.to_color32().gamma_multiply(0.5);
        painter.line_segment(
            [
                Pos2::new(rect.min.x, rect.center().y),
                Pos2::new(rect.max.x, rect.center().y),
            ],
            Stroke::new(0.5, muted),
        );
    }

    if peaks.is_empty() {
        return response;
    }
    let cx = rect.center().y;
    let hh = rect.height() * 0.5;
    let bar_w = rect.width() / peaks.len() as f32;
    for (i, (min, max)) in peaks.iter().enumerate() {
        let x = rect.min.x + i as f32 * bar_w + bar_w * 0.5;
        let y_top = cx - hh * max.clamp(-1.0, 1.0);
        let y_bot = cx - hh * min.clamp(-1.0, 1.0);
        painter.line_segment(
            [Pos2::new(x, y_top), Pos2::new(x, y_bot)],
            Stroke::new(bar_w.max(1.0), color),
        );
    }
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );

    response
}
