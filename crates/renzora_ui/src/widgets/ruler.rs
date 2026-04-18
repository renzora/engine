//! Pixel ruler — major/minor ticks with numeric labels, horizontal or vertical.

use bevy_egui::egui::{self, Pos2, Sense, Stroke, Vec2};
use renzora_theme::Theme;

#[derive(Clone, Copy, Debug)]
pub enum RulerAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug)]
pub struct RulerConfig {
    pub axis: RulerAxis,
    pub length: f32,
    pub thickness: f32,
    /// How many units equal one minor tick.
    pub minor_step: f32,
    /// How many minor ticks between major ticks.
    pub major_every: u32,
    /// World-space origin in the same units as `minor_step`.
    pub origin: f32,
    /// Pixels per unit.
    pub pixels_per_unit: f32,
}

impl Default for RulerConfig {
    fn default() -> Self {
        Self {
            axis: RulerAxis::Horizontal,
            length: 200.0,
            thickness: 16.0,
            minor_step: 10.0,
            major_every: 5,
            origin: 0.0,
            pixels_per_unit: 1.0,
        }
    }
}

/// Render a ruler. Returns the allocated response.
pub fn ruler(ui: &mut egui::Ui, cfg: RulerConfig, theme: &Theme) -> egui::Response {
    let (size, sense) = match cfg.axis {
        RulerAxis::Horizontal => (Vec2::new(cfg.length, cfg.thickness), Sense::hover()),
        RulerAxis::Vertical => (Vec2::new(cfg.thickness, cfg.length), Sense::hover()),
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme.surfaces.faint.to_color32());

    let minor_px = cfg.minor_step * cfg.pixels_per_unit;
    let primary = theme.text.primary.to_color32();
    let muted = theme.text.muted.to_color32();

    match cfg.axis {
        RulerAxis::Horizontal => {
            let y_major = rect.max.y;
            let y_minor_len = cfg.thickness * 0.35;
            let y_major_len = cfg.thickness * 0.7;
            let mut i = 0i32;
            let mut x = rect.min.x;
            while x <= rect.max.x {
                let is_major = i % cfg.major_every as i32 == 0;
                let len = if is_major { y_major_len } else { y_minor_len };
                painter.line_segment(
                    [Pos2::new(x, y_major - len), Pos2::new(x, y_major)],
                    Stroke::new(1.0, if is_major { primary } else { muted }),
                );
                if is_major {
                    let value = cfg.origin + i as f32 * cfg.minor_step;
                    painter.text(
                        Pos2::new(x + 2.0, rect.min.y + 1.0),
                        egui::Align2::LEFT_TOP,
                        format!("{:.0}", value),
                        egui::FontId::proportional(9.0),
                        muted,
                    );
                }
                i += 1;
                x += minor_px;
            }
        }
        RulerAxis::Vertical => {
            let x_major = rect.max.x;
            let x_minor_len = cfg.thickness * 0.35;
            let x_major_len = cfg.thickness * 0.7;
            let mut i = 0i32;
            let mut y = rect.min.y;
            while y <= rect.max.y {
                let is_major = i % cfg.major_every as i32 == 0;
                let len = if is_major { x_major_len } else { x_minor_len };
                painter.line_segment(
                    [Pos2::new(x_major - len, y), Pos2::new(x_major, y)],
                    Stroke::new(1.0, if is_major { primary } else { muted }),
                );
                if is_major {
                    let value = cfg.origin + i as f32 * cfg.minor_step;
                    painter.text(
                        Pos2::new(rect.min.x + 1.0, y + 1.0),
                        egui::Align2::LEFT_TOP,
                        format!("{:.0}", value),
                        egui::FontId::proportional(9.0),
                        muted,
                    );
                }
                i += 1;
                y += minor_px;
            }
        }
    }

    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );
    response
}
