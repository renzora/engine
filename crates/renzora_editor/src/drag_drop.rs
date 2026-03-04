//! Drag-and-drop state and drop target detection for tab rearrangement.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use renzora_theme::Theme;

use crate::dock_tree::DropZone;

/// Active drag state — inserted as a resource when a drag begins, removed on drop/cancel.
#[derive(Resource)]
pub struct DragState {
    /// Which tab is being dragged.
    pub panel_id: String,
    /// Where the drag started (screen coords).
    pub origin: Pos2,
    /// True once pointer moves >5px from origin (prevents accidental drags on click).
    pub is_detached: bool,
}

/// Computed each frame during drag — which drop target is the pointer over.
pub struct DropTarget {
    /// Target leaf's active panel ID (used for tree lookup).
    pub panel_id: String,
    /// Where in the target to drop.
    pub zone: DropZone,
    /// Rect to highlight as visual feedback.
    pub visual_rect: Rect,
}

const TAB_BAR_HEIGHT: f32 = 28.0;
const EDGE_FRACTION: f32 = 0.25;

/// Detect which drop zone the pointer is over relative to a leaf rect and its tab rects.
pub fn detect_drop_target(
    pointer: Pos2,
    leaf_rect: Rect,
    tabs: &[String],
    tab_rects: &[Rect],
    dragged_panel: &str,
) -> Option<DropTarget> {
    if !leaf_rect.contains(pointer) {
        return None;
    }

    // Don't allow dropping onto a single-tab leaf that is the source
    if tabs.len() == 1 && tabs[0] == dragged_panel {
        return None;
    }

    let tab_bar_rect = Rect::from_min_size(leaf_rect.min, Vec2::new(leaf_rect.width(), TAB_BAR_HEIGHT));

    // Check tab bar area for tab insertion
    if tab_bar_rect.contains(pointer) {
        // Find insertion index between tab rects
        for (i, tr) in tab_rects.iter().enumerate() {
            let mid_x = tr.center().x;
            if pointer.x < mid_x {
                let marker_rect = Rect::from_min_size(
                    Pos2::new(tr.min.x - 1.0, tr.min.y),
                    Vec2::new(2.0, tr.height()),
                );
                return Some(DropTarget {
                    panel_id: first_non_dragged(tabs, dragged_panel),
                    zone: DropZone::Tab(i),
                    visual_rect: marker_rect,
                });
            }
        }
        // After last tab
        let idx = tabs.len();
        let visual_rect = if let Some(last) = tab_rects.last() {
            Rect::from_min_size(
                Pos2::new(last.max.x - 1.0, last.min.y),
                Vec2::new(2.0, last.height()),
            )
        } else {
            Rect::from_min_size(leaf_rect.min, Vec2::new(2.0, TAB_BAR_HEIGHT))
        };
        return Some(DropTarget {
            panel_id: first_non_dragged(tabs, dragged_panel),
            zone: DropZone::Tab(idx),
            visual_rect,
        });
    }

    // Content area — check edge zones
    let content_rect = Rect::from_min_max(
        Pos2::new(leaf_rect.min.x, leaf_rect.min.y + TAB_BAR_HEIGHT),
        leaf_rect.max,
    );

    if !content_rect.contains(pointer) {
        return None;
    }

    let w = content_rect.width();
    let h = content_rect.height();
    let rel_x = pointer.x - content_rect.min.x;
    let rel_y = pointer.y - content_rect.min.y;

    let target_id = first_non_dragged(tabs, dragged_panel);

    // Check edges (outer 25%)
    if rel_x < w * EDGE_FRACTION {
        return Some(DropTarget {
            panel_id: target_id,
            zone: DropZone::Left,
            visual_rect: Rect::from_min_size(
                content_rect.min,
                Vec2::new(w * 0.5, h),
            ),
        });
    }
    if rel_x > w * (1.0 - EDGE_FRACTION) {
        return Some(DropTarget {
            panel_id: target_id,
            zone: DropZone::Right,
            visual_rect: Rect::from_min_size(
                Pos2::new(content_rect.min.x + w * 0.5, content_rect.min.y),
                Vec2::new(w * 0.5, h),
            ),
        });
    }
    if rel_y < h * EDGE_FRACTION {
        return Some(DropTarget {
            panel_id: target_id,
            zone: DropZone::Top,
            visual_rect: Rect::from_min_size(
                content_rect.min,
                Vec2::new(w, h * 0.5),
            ),
        });
    }
    if rel_y > h * (1.0 - EDGE_FRACTION) {
        return Some(DropTarget {
            panel_id: target_id,
            zone: DropZone::Bottom,
            visual_rect: Rect::from_min_size(
                Pos2::new(content_rect.min.x, content_rect.min.y + h * 0.5),
                Vec2::new(w, h * 0.5),
            ),
        });
    }

    // Center — add as tab
    Some(DropTarget {
        panel_id: target_id,
        zone: DropZone::Center,
        visual_rect: content_rect,
    })
}

/// Draw a floating ghost label following the pointer during drag.
pub fn draw_drag_ghost(ctx: &egui::Context, panel_title: &str, pointer: Pos2, theme: &Theme) {
    let offset = Vec2::new(12.0, -8.0);
    let pos = pointer + offset;

    egui::Area::new(egui::Id::new("drag_ghost"))
        .fixed_pos(pos)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::NONE
                .fill(theme.surfaces.panel.to_color32())
                .stroke(Stroke::new(1.0, theme.semantic.selection.to_color32()))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .corner_radius(egui::CornerRadius::same(4));
            frame.show(ui, |ui| {
                ui.label(
                    egui::RichText::new(panel_title)
                        .font(FontId::proportional(11.0))
                        .color(theme.text.primary.to_color32()),
                );
            });
        });
}

/// Draw a tab insertion marker — glow + solid accent line + triangles at endpoints.
pub fn draw_tab_insert_marker(ui: &mut egui::Ui, rect: Rect, theme: &Theme) {
    let accent = theme.semantic.accent.to_color32();
    let x = rect.center().x;
    let top = Pos2::new(x, rect.min.y + 2.0);
    let bottom = Pos2::new(x, rect.max.y - 2.0);

    // Glow layer (6px, 24% opacity)
    let glow = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 60);
    ui.painter().line_segment([top, bottom], Stroke::new(6.0, glow));

    // Main line (3px solid)
    ui.painter().line_segment([top, bottom], Stroke::new(3.0, accent));

    // Top triangle
    let tri = 5.0;
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(x, top.y),
            Pos2::new(x - tri, top.y - tri),
            Pos2::new(x + tri, top.y - tri),
        ],
        accent,
        Stroke::NONE,
    ));

    // Bottom triangle
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(x, bottom.y),
            Pos2::new(x - tri, bottom.y + tri),
            Pos2::new(x + tri, bottom.y + tri),
        ],
        accent,
        Stroke::NONE,
    ));
}

/// Draw a directional drop zone overlay (semi-transparent tinted rectangle).
pub fn draw_zone_overlay(ui: &mut egui::Ui, rect: Rect, theme: &Theme) {
    let accent = theme.semantic.accent.to_color32();
    let fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 50);
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0, accent), egui::StrokeKind::Inside);
}

/// Get the first panel ID in tabs that isn't the dragged one (for tree lookups).
fn first_non_dragged(tabs: &[String], dragged: &str) -> String {
    tabs.iter()
        .find(|t| t.as_str() != dragged)
        .cloned()
        .unwrap_or_else(|| tabs.first().cloned().unwrap_or_default())
}
