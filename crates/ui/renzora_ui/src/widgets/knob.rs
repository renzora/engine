//! Rotary knob — 270° arc dial for continuous parameter control.

use bevy_egui::egui::{self, Color32, CursorIcon, Pos2, Sense, Stroke, Vec2};

use super::colors::dim_color;

/// Configuration for a rotary knob.
pub struct KnobConfig {
    /// Diameter of the knob in pixels.
    pub size: f32,
    /// Minimum value.
    pub min: f32,
    /// Maximum value.
    pub max: f32,
    /// Accent color for the value arc and indicator.
    pub color: Color32,
    /// Color for the background track arc.
    pub track_color: Color32,
    /// Optional label drawn below the knob.
    pub label: Option<String>,
}

impl Default for KnobConfig {
    fn default() -> Self {
        Self {
            size: 48.0,
            min: 0.0,
            max: 1.0,
            color: Color32::from_rgb(100, 180, 100),
            track_color: Color32::from_rgb(30, 36, 30),
            label: None,
        }
    }
}

/// Map a normalized parameter `t` ∈ [0, 1] to a point on the 270° arc.
///
/// The arc starts at 135° (bottom-left) and sweeps 270° clockwise to 45° (bottom-right).
fn knob_point(center: Pos2, radius: f32, t: f32) -> Pos2 {
    let start_angle = std::f32::consts::FRAC_PI_4 * 3.0; // 135°
    let sweep = std::f32::consts::FRAC_PI_2 * 3.0; // 270°
    let angle = start_angle + sweep * t;
    Pos2::new(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
    )
}

/// Generate points along the 270° arc for polyline rendering.
fn arc_points(center: Pos2, radius: f32, from_t: f32, to_t: f32, segments: usize) -> Vec<Pos2> {
    (0..=segments)
        .map(|i| {
            let t = from_t + (to_t - from_t) * (i as f32 / segments as f32);
            knob_point(center, radius, t)
        })
        .collect()
}

/// Paint an interactive rotary knob. Returns `true` if the value changed.
///
/// - Drag up/down to change value
/// - Hold Shift for fine control (10× slower)
/// - Double-click to reset to midpoint
/// - Hover shows tooltip with current value
pub fn rotary_knob(
    ui: &mut egui::Ui,
    _id: egui::Id,
    value: &mut f32,
    config: &KnobConfig,
) -> bool {
    let total_height = if config.label.is_some() {
        config.size + 14.0
    } else {
        config.size
    };
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(config.size, total_height),
        Sense::click_and_drag(),
    );

    let mut changed = false;
    let range = config.max - config.min;

    // Double-click → reset to midpoint
    if response.double_clicked() {
        *value = config.min + range * 0.5;
        changed = true;
    }

    // Drag up/down to change
    if response.dragged() {
        let delta = -response.drag_delta().y;
        let speed = if ui.input(|i| i.modifiers.shift) {
            0.001
        } else {
            0.005
        };
        *value += delta * range * speed;
        *value = value.clamp(config.min, config.max);
        changed = true;
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
        response.clone().on_hover_text(format!("{:.2}", *value));
    }

    // ── Paint ──────────────────────────────────────────────────────

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let knob_rect = egui::Rect::from_min_size(rect.min, Vec2::splat(config.size));
        let center = knob_rect.center();
        let outer_r = config.size * 0.5 - 2.0;
        let arc_r = outer_r - 4.0;
        let t = (*value - config.min) / range;

        // Track arc (full 270°)
        let track_pts = arc_points(center, arc_r, 0.0, 1.0, 40);
        painter.add(egui::Shape::line(
            track_pts,
            Stroke::new(3.0, config.track_color),
        ));

        // Value arc
        if t > 0.005 {
            let val_pts = arc_points(center, arc_r, 0.0, t, (40.0 * t) as usize + 2);
            painter.add(egui::Shape::line(
                val_pts,
                Stroke::new(3.0, config.color),
            ));
        }

        // Knob body
        let body_r = outer_r - 8.0;
        let body_color = if response.hovered() || response.dragged() {
            Color32::from_rgb(55, 58, 65)
        } else {
            Color32::from_rgb(45, 48, 55)
        };
        painter.circle_filled(center, body_r, body_color);
        painter.circle_stroke(center, body_r, Stroke::new(1.0, Color32::from_rgb(30, 30, 35)));

        // Indicator dot
        let dot_pos = knob_point(center, body_r - 4.0, t);
        painter.circle_filled(dot_pos, 3.0, config.color);

        // Label
        if let Some(ref label) = config.label {
            painter.text(
                Pos2::new(center.x, knob_rect.bottom() + 8.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(10.0),
                dim_color(config.color, 0.8),
            );
        }
    }

    changed
}
