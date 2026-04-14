//! Generic drop-zone highlight helper.
//!
//! Paints a tinted fill + accent border on a rect when it is a valid drop
//! target for an in-flight drag. Extracted so any widget (property row,
//! custom inspector, game UI editor, timeline track) can advertise
//! drop-target feedback consistently.

use bevy_egui::egui::{self, Color32, Rect, Stroke};

/// Draw drop-zone feedback on `rect`. No-op when `is_valid_target` is false.
///
/// `accent` is the color used for the border + tinted fill (typically the
/// dragged payload's color, or the theme accent). `active` signals that the
/// pointer is currently over the rect — border thickens and the fill tint
/// becomes more prominent.
pub fn paint_drop_highlight(
    ui: &egui::Ui,
    rect: Rect,
    is_valid_target: bool,
    active: bool,
    accent: Color32,
    corner_radius: f32,
) {
    if !is_valid_target {
        return;
    }
    let fill_alpha = if active { 40 } else { 18 };
    let fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), fill_alpha);
    ui.painter().rect_filled(rect, corner_radius, fill);

    let stroke_width = if active { 1.5 } else { 1.0 };
    ui.painter().rect_stroke(
        rect,
        corner_radius,
        Stroke::new(stroke_width, accent),
        egui::StrokeKind::Inside,
    );
}
