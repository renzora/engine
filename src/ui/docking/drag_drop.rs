//! Drag and drop handling for the docking system
//!
//! Manages the drag state and drop zone detection for panels.

use super::dock_tree::{DropZone, PanelId};
use bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind};
use crate::theming::Theme;

/// State for an ongoing drag operation
#[derive(Debug, Clone)]
pub struct DragState {
    /// The panel being dragged
    pub panel: PanelId,
    /// The starting position of the drag
    pub start_pos: Pos2,
    /// Current drop target (if any)
    pub drop_target: Option<DropTarget>,
}

/// Describes where a panel will be dropped
#[derive(Debug, Clone)]
pub struct DropTarget {
    /// The leaf/panel being dropped onto
    pub target_panel: PanelId,
    /// The zone where the drop will occur
    pub zone: DropZone,
    /// The rectangle of the drop zone (for visual feedback)
    pub rect: Rect,
}

impl DragState {
    pub fn new(panel: PanelId, start_pos: Pos2) -> Self {
        Self {
            panel,
            start_pos,
            drop_target: None,
        }
    }
}

/// Size of the drop zone areas at panel edges
const DROP_ZONE_SIZE: f32 = 60.0;
/// Minimum drag distance before considering it a drag (not a click)
const MIN_DRAG_DISTANCE: f32 = 5.0;

/// Determine which drop zone (if any) the cursor is in
pub fn detect_drop_zone(cursor_pos: Pos2, panel_rect: Rect) -> Option<DropZone> {
    if !panel_rect.contains(cursor_pos) {
        return None;
    }

    let center = panel_rect.center();
    let rel_x = cursor_pos.x - panel_rect.min.x;
    let rel_y = cursor_pos.y - panel_rect.min.y;
    let width = panel_rect.width();
    let height = panel_rect.height();

    // Check if in center zone (for tab drop)
    let center_rect = Rect::from_center_size(center, egui::vec2(width * 0.3, height * 0.3));
    if center_rect.contains(cursor_pos) {
        return Some(DropZone::Tab);
    }

    // Check edge zones
    let edge_size = DROP_ZONE_SIZE.min(width * 0.25).min(height * 0.25);

    if rel_x < edge_size {
        Some(DropZone::Left)
    } else if rel_x > width - edge_size {
        Some(DropZone::Right)
    } else if rel_y < edge_size {
        Some(DropZone::Top)
    } else if rel_y > height - edge_size {
        Some(DropZone::Bottom)
    } else {
        // In the panel but not in any specific zone - default to tab
        Some(DropZone::Tab)
    }
}

/// Get the rectangle that will be highlighted for a drop zone
pub fn get_drop_zone_rect(zone: DropZone, panel_rect: Rect) -> Rect {
    let width = panel_rect.width();
    let height = panel_rect.height();

    match zone {
        DropZone::Tab => {
            // Highlight the whole panel slightly
            panel_rect.shrink(4.0)
        }
        DropZone::Left => {
            Rect::from_min_size(panel_rect.min, egui::vec2(width * 0.5, height))
        }
        DropZone::Right => {
            let half_width = width * 0.5;
            Rect::from_min_size(
                Pos2::new(panel_rect.min.x + half_width, panel_rect.min.y),
                egui::vec2(half_width, height),
            )
        }
        DropZone::Top => {
            Rect::from_min_size(panel_rect.min, egui::vec2(width, height * 0.5))
        }
        DropZone::Bottom => {
            let half_height = height * 0.5;
            Rect::from_min_size(
                Pos2::new(panel_rect.min.x, panel_rect.min.y + half_height),
                egui::vec2(width, half_height),
            )
        }
    }
}

/// Draw drop zone overlay for visual feedback during drag
pub fn draw_drop_zone_overlay(ui: &egui::Ui, zone: DropZone, panel_rect: Rect, theme: &Theme) {
    let drop_rect = get_drop_zone_rect(zone, panel_rect);

    // Use theme accent color for drop zone
    let accent = theme.semantic.accent.to_color32();
    let [r, g, b, _] = accent.to_array();
    let fill_color = Color32::from_rgba_unmultiplied(r, g, b, 80);
    let stroke_color = Color32::from_rgba_unmultiplied(r, g, b, 200);

    ui.painter().rect(
        drop_rect,
        4.0,
        fill_color,
        Stroke::new(2.0, stroke_color),
        StrokeKind::Outside,
    );

    // Draw an icon in the center indicating what will happen
    let icon = match zone {
        DropZone::Tab => "\u{f24d}", // clone (layers icon)
        DropZone::Left | DropZone::Right => "\u{f0c9}", // bars (horizontal)
        DropZone::Top | DropZone::Bottom => "\u{f0c9}", // bars
    };

    ui.painter().text(
        drop_rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(24.0),
        stroke_color,
    );
}

/// Draw the dragged tab preview following the cursor
pub fn draw_drag_preview(ui: &egui::Ui, panel: &PanelId, cursor_pos: Pos2, theme: &Theme) {
    let text = format!("{} {}", panel.icon(), panel.title());
    let rect = Rect::from_center_size(
        cursor_pos + egui::vec2(10.0, 10.0),
        egui::vec2(120.0, 28.0),
    );

    let popup_bg = theme.surfaces.popup.to_color32();
    let [r, g, b, _] = popup_bg.to_array();
    let bg_color = Color32::from_rgba_unmultiplied(r, g, b, 230);

    ui.painter().rect(
        rect,
        4.0,
        bg_color,
        Stroke::new(1.0, theme.semantic.accent.to_color32()),
        StrokeKind::Outside,
    );

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
}

/// Check if the drag has exceeded the minimum distance threshold
pub fn is_drag_active(start_pos: Pos2, current_pos: Pos2) -> bool {
    start_pos.distance(current_pos) >= MIN_DRAG_DISTANCE
}
