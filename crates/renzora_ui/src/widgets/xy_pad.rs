//! 2D XY pad — click/drag a crosshair over a normalized (x, y) range.
//!
//! Useful for stereo pan, aim direction, 2D joystick preview, normalized
//! surface selection.

use bevy_egui::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

/// Configuration for `xy_pad`.
#[derive(Clone, Copy, Debug)]
pub struct XyPadConfig {
    pub size: f32,
    pub x_range: (f32, f32),
    pub y_range: (f32, f32),
    /// Draw center crosshair grid lines.
    pub show_center: bool,
}

impl Default for XyPadConfig {
    fn default() -> Self {
        Self { size: 120.0, x_range: (-1.0, 1.0), y_range: (-1.0, 1.0), show_center: true }
    }
}

/// XY pad. Mutates `x` and `y` in place.
pub fn xy_pad(
    ui: &mut egui::Ui,
    x: &mut f32,
    y: &mut f32,
    cfg: XyPadConfig,
    theme: &Theme,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(cfg.size), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme.surfaces.faint.to_color32());
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );

    if cfg.show_center {
        let c = rect.center();
        let muted = theme.widgets.border.to_color32().gamma_multiply(0.5);
        painter.line_segment(
            [Pos2::new(rect.min.x, c.y), Pos2::new(rect.max.x, c.y)],
            Stroke::new(0.5, muted),
        );
        painter.line_segment(
            [Pos2::new(c.x, rect.min.y), Pos2::new(c.x, rect.max.y)],
            Stroke::new(0.5, muted),
        );
    }

    let (xmin, xmax) = cfg.x_range;
    let (ymin, ymax) = cfg.y_range;
    let to_screen = |vx: f32, vy: f32| {
        Pos2::new(
            rect.min.x + rect.width() * (vx - xmin) / (xmax - xmin),
            rect.max.y - rect.height() * (vy - ymin) / (ymax - ymin),
        )
    };

    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            *x = (xmin + (pos.x - rect.min.x) / rect.width() * (xmax - xmin)).clamp(xmin, xmax);
            *y = (ymin + (rect.max.y - pos.y) / rect.height() * (ymax - ymin)).clamp(ymin, ymax);
        }
    }

    let dot = to_screen(*x, *y);
    painter.circle_filled(dot, 5.0, theme.widgets.active_bg.to_color32());
    painter.circle_stroke(dot, 5.0, Stroke::new(1.0, Color32::WHITE));

    response
}
