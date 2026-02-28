//! Drag and drop handling for the docking system
//!
//! Manages the drag state and drop zone detection for panels.

use super::dock_tree::{DropZone, PanelId};
use bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind};
use renzora_theme::Theme;

/// State for an ongoing drag operation
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DragState {
    /// The panel being dragged
    pub panel: PanelId,
    /// The starting position of the drag
    pub start_pos: Pos2,
    /// The original rect of the panel being dragged
    pub original_rect: Rect,
    /// Offset from cursor to panel top-left (for smooth dragging)
    pub drag_offset: egui::Vec2,
    /// Current drop target (if any)
    pub drop_target: Option<DropTarget>,
    /// Animation progress for smooth transitions (0.0 to 1.0)
    pub animation_progress: f32,
    /// Last target panel and zone for detecting changes (to reset animation)
    pub last_target: Option<(PanelId, DropZone)>,
}

/// Describes where a panel will be dropped
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DropTarget {
    /// The leaf/panel being dropped onto
    pub target_panel: PanelId,
    /// The zone where the drop will occur
    pub zone: DropZone,
    /// The rectangle of the drop zone (for visual feedback)
    pub rect: Rect,
}

impl DragState {
    pub fn new(panel: PanelId, start_pos: Pos2, panel_rect: Rect) -> Self {
        // Calculate offset from cursor to panel top-left
        let drag_offset = panel_rect.min - start_pos;
        Self {
            panel,
            start_pos,
            original_rect: panel_rect,
            drag_offset,
            drop_target: None,
            animation_progress: 0.0,
            last_target: None,
        }
    }

    /// Update the current target and reset animation if target changed
    pub fn update_target(&mut self, target_panel: Option<PanelId>, zone: Option<DropZone>) {
        let new_target = match (target_panel, zone) {
            (Some(panel), Some(z)) => Some((panel, z)),
            _ => None,
        };

        if new_target != self.last_target {
            self.animation_progress = 0.0;
            self.last_target = new_target;
        }
    }
}

/// Information about a tab position for insertion indicator
#[derive(Debug, Clone)]
pub struct TabInsertInfo {
    /// X position where the insertion indicator should be drawn
    pub insert_x: f32,
    /// Index in the tab list where the panel will be inserted
    #[allow(dead_code)]
    pub insert_index: usize,
}

/// Detect where in the tab bar the cursor is for precise tab insertion
pub fn detect_tab_insert_position(
    cursor_pos: Pos2,
    tab_bar_rect: Rect,
    tab_positions: &[(Rect, PanelId)],
) -> Option<TabInsertInfo> {
    if !tab_bar_rect.contains(cursor_pos) {
        return None;
    }

    // Find which tab gap the cursor is closest to
    if tab_positions.is_empty() {
        return Some(TabInsertInfo {
            insert_x: tab_bar_rect.min.x + 4.0,
            insert_index: 0,
        });
    }

    // Check if before first tab
    if let Some((first_tab, _)) = tab_positions.first() {
        if cursor_pos.x < first_tab.center().x {
            return Some(TabInsertInfo {
                insert_x: first_tab.min.x,
                insert_index: 0,
            });
        }
    }

    // Check between tabs and after last tab
    for (i, (tab_rect, _)) in tab_positions.iter().enumerate() {
        let next_index = i + 1;
        if next_index < tab_positions.len() {
            let next_rect = &tab_positions[next_index].0;
            let mid_x = (tab_rect.max.x + next_rect.min.x) / 2.0;
            if cursor_pos.x < mid_x {
                return Some(TabInsertInfo {
                    insert_x: tab_rect.max.x + 1.0,
                    insert_index: next_index,
                });
            }
        } else {
            // After last tab
            return Some(TabInsertInfo {
                insert_x: tab_rect.max.x + 1.0,
                insert_index: next_index,
            });
        }
    }

    None
}

/// Minimum drag distance before considering it a drag (not a click)
#[allow(dead_code)]
const MIN_DRAG_DISTANCE: f32 = 5.0;

/// Determine which drop zone (if any) the cursor is in
/// The zones match the highlighted areas - once you enter a zone, the entire
/// highlighted region keeps that zone active.
pub fn detect_drop_zone(cursor_pos: Pos2, panel_rect: Rect) -> Option<DropZone> {
    if !panel_rect.contains(cursor_pos) {
        return None;
    }

    let center = panel_rect.center();
    let rel_x = cursor_pos.x - panel_rect.min.x;
    let rel_y = cursor_pos.y - panel_rect.min.y;
    let width = panel_rect.width();
    let height = panel_rect.height();

    // Check if in center zone (for tab drop) - a smaller central area
    let center_rect = Rect::from_center_size(center, egui::vec2(width * 0.3, height * 0.3));
    if center_rect.contains(cursor_pos) {
        return Some(DropZone::Tab);
    }

    // Use half-panel zones that match the highlighted areas
    // Determine which half of the panel the cursor is in
    let in_left_half = rel_x < width * 0.5;
    let in_top_half = rel_y < height * 0.5;

    // Calculate distance from each edge as a ratio
    let left_ratio = rel_x / width;
    let right_ratio = 1.0 - left_ratio;
    let top_ratio = rel_y / height;
    let bottom_ratio = 1.0 - top_ratio;

    // Find the closest edge
    let min_ratio = left_ratio.min(right_ratio).min(top_ratio).min(bottom_ratio);

    if min_ratio == left_ratio && in_left_half {
        Some(DropZone::Left)
    } else if min_ratio == right_ratio && !in_left_half {
        Some(DropZone::Right)
    } else if min_ratio == top_ratio && in_top_half {
        Some(DropZone::Top)
    } else if min_ratio == bottom_ratio && !in_top_half {
        Some(DropZone::Bottom)
    } else if in_left_half && in_top_half {
        // Tiebreaker: prefer horizontal splits for corners
        if left_ratio < top_ratio { Some(DropZone::Left) } else { Some(DropZone::Top) }
    } else if !in_left_half && in_top_half {
        if right_ratio < top_ratio { Some(DropZone::Right) } else { Some(DropZone::Top) }
    } else if in_left_half && !in_top_half {
        if left_ratio < bottom_ratio { Some(DropZone::Left) } else { Some(DropZone::Bottom) }
    } else {
        if right_ratio < bottom_ratio { Some(DropZone::Right) } else { Some(DropZone::Bottom) }
    }
}

/// Draw tab insertion indicator - a vertical line showing where the tab will be inserted
pub fn draw_tab_insert_indicator(ui: &egui::Ui, insert_info: &TabInsertInfo, tab_bar_rect: Rect, theme: &Theme) {
    let accent = theme.semantic.accent.to_color32();

    // Draw a prominent vertical line where the tab will be inserted
    let line_height = tab_bar_rect.height() - 4.0;
    let top = Pos2::new(insert_info.insert_x, tab_bar_rect.min.y + 2.0);
    let bottom = Pos2::new(insert_info.insert_x, tab_bar_rect.min.y + 2.0 + line_height);

    // Draw glow effect
    ui.painter().line_segment(
        [top, bottom],
        Stroke::new(6.0, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 60)),
    );

    // Draw main line
    ui.painter().line_segment(
        [top, bottom],
        Stroke::new(3.0, accent),
    );

    // Draw small triangles at top and bottom
    let tri_size = 5.0;

    // Top triangle
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(insert_info.insert_x - tri_size, top.y),
            Pos2::new(insert_info.insert_x + tri_size, top.y),
            Pos2::new(insert_info.insert_x, top.y + tri_size),
        ],
        accent,
        Stroke::NONE,
    ));

    // Bottom triangle
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(insert_info.insert_x - tri_size, bottom.y),
            Pos2::new(insert_info.insert_x + tri_size, bottom.y),
            Pos2::new(insert_info.insert_x, bottom.y - tri_size),
        ],
        accent,
        Stroke::NONE,
    ));
}

/// Draw the dragged tab preview following the cursor
#[allow(dead_code)]
pub fn draw_drag_preview(ui: &egui::Ui, panel: &PanelId, cursor_pos: Pos2, theme: &Theme) {
    let text = format!("{} {}", panel.icon(), panel.localized_title());
    let rect = Rect::from_center_size(
        cursor_pos + egui::vec2(10.0, 10.0),
        egui::vec2(120.0, 28.0),
    );

    let popup_bg = theme.surfaces.popup.to_color32();
    let [r, g, b, _] = popup_bg.to_array();
    let bg_color = Color32::from_rgba_unmultiplied(r, g, b, 230);

    ui.painter().rect_filled(rect, 4.0, bg_color);
    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, theme.semantic.accent.to_color32()), StrokeKind::Outside);

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
}

/// Check if the drag has exceeded the minimum distance threshold
#[allow(dead_code)]
pub fn is_drag_active(start_pos: Pos2, current_pos: Pos2) -> bool {
    start_pos.distance(current_pos) >= MIN_DRAG_DISTANCE
}
