//! Vertical fader — mixer-style vertical slider with groove, handle, and tick marks.

use bevy_egui::egui::{self, Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Vec2};

/// Configuration for a vertical fader.
pub struct FaderConfig {
    /// Total width of the fader area.
    pub width: f32,
    /// Total height of the fader area.
    pub height: f32,
    /// Minimum value (bottom).
    pub min: f32,
    /// Maximum value (top).
    pub max: f32,
    /// Color for the groove track.
    pub track_color: Color32,
    /// Handle cap color.
    pub handle_color: Color32,
    /// Optional label drawn below the fader.
    pub label: Option<String>,
    /// Color for the label text.
    pub label_color: Color32,
}

impl Default for FaderConfig {
    fn default() -> Self {
        Self {
            width: 32.0,
            height: 140.0,
            min: 0.0,
            max: 1.0,
            track_color: Color32::from_rgb(15, 15, 18),
            handle_color: Color32::from_rgb(45, 48, 55),
            label: None,
            label_color: Color32::from_rgb(140, 142, 148),
        }
    }
}

/// Paint an interactive vertical fader. Returns `true` if the value changed.
///
/// - Drag handle vertically to change value
/// - Click anywhere on the groove to snap
pub fn vertical_fader(
    ui: &mut egui::Ui,
    _id: egui::Id,
    value: &mut f32,
    config: &FaderConfig,
) -> bool {
    let total_height = if config.label.is_some() {
        config.height + 16.0
    } else {
        config.height
    };
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(config.width, total_height),
        Sense::click_and_drag(),
    );

    let mut changed = false;
    let range = config.max - config.min;
    let handle_h = 12.0;
    let groove_top = rect.top() + handle_h * 0.5;
    let groove_bottom = rect.top() + config.height - handle_h * 0.5;
    let groove_range = groove_bottom - groove_top;

    // Click or drag to set value
    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = 1.0 - ((pos.y - groove_top) / groove_range).clamp(0.0, 1.0);
            *value = config.min + t * range;
            changed = true;
        }
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
    }

    // ── Paint ──────────────────────────────────────────────────────

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center_x = rect.center().x;
        let t = (*value - config.min) / range;
        let handle_y = groove_bottom - t * groove_range;

        // Groove (4px wide)
        let groove_rect = Rect::from_center_size(
            Pos2::new(center_x, (groove_top + groove_bottom) * 0.5),
            Vec2::new(4.0, groove_range),
        );
        painter.rect_filled(groove_rect, 2.0, config.track_color);

        // Filled region below handle
        if t > 0.005 {
            let fill_rect = Rect::from_min_max(
                Pos2::new(center_x - 2.0, handle_y),
                Pos2::new(center_x + 2.0, groove_bottom),
            );
            let fill_color = Color32::from_rgb(80, 160, 80);
            painter.rect_filled(fill_rect, 2.0, fill_color);
        }

        // Tick marks (5 evenly spaced)
        for i in 0..=4 {
            let tick_t = i as f32 / 4.0;
            let tick_y = groove_bottom - tick_t * groove_range;
            let tick_left = center_x - config.width * 0.35;
            let tick_right = center_x - 5.0;
            painter.line_segment(
                [Pos2::new(tick_left, tick_y), Pos2::new(tick_right, tick_y)],
                Stroke::new(1.0, Color32::from_rgb(60, 60, 65)),
            );
        }

        // Handle cap
        let handle_color = if response.hovered() || response.dragged() {
            Color32::from_rgb(65, 68, 78)
        } else {
            config.handle_color
        };
        let handle_rect = Rect::from_center_size(
            Pos2::new(center_x, handle_y),
            Vec2::new(config.width - 8.0, handle_h),
        );
        painter.rect_filled(handle_rect, 3.0, handle_color);
        painter.rect_stroke(handle_rect, 3.0, Stroke::new(1.0, Color32::from_rgb(30, 30, 35)), egui::StrokeKind::Outside);
        // Handle center line
        painter.line_segment(
            [
                Pos2::new(handle_rect.left() + 4.0, handle_y),
                Pos2::new(handle_rect.right() - 4.0, handle_y),
            ],
            Stroke::new(1.0, Color32::from_rgb(100, 100, 110)),
        );

        // Label
        if let Some(ref label) = config.label {
            painter.text(
                Pos2::new(center_x, rect.top() + config.height + 8.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(10.0),
                config.label_color,
            );
        }
    }

    changed
}
