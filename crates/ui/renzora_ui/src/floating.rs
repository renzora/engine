//! Floating (undocked) panel windows.
//!
//! Panels dragged outside the dock tree become floating windows that can be
//! freely repositioned and resized. They can be re-docked via right-click → "Dock".

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, Id, Order, Pos2, Rect, Sense, Stroke, Vec2};
use renzora_theme::Theme;

use crate::panel::PanelRegistry;

/// A single floating (undocked) panel window.
#[derive(Debug, Clone)]
pub struct FloatingPanel {
    pub panel_id: String,
    pub pos: Pos2,
    pub size: Vec2,
}

/// Resource holding all floating (undocked) panels.
#[derive(Resource, Default)]
pub struct FloatingPanels {
    pub panels: Vec<FloatingPanel>,
}

impl FloatingPanels {
    pub fn add(&mut self, panel_id: String, pos: Pos2, size: Vec2) {
        if self.panels.iter().any(|p| p.panel_id == panel_id) {
            return;
        }
        self.panels.push(FloatingPanel { panel_id, pos, size });
    }

    pub fn remove(&mut self, panel_id: &str) -> Option<FloatingPanel> {
        if let Some(idx) = self.panels.iter().position(|p| p.panel_id == panel_id) {
            Some(self.panels.remove(idx))
        } else {
            None
        }
    }

    pub fn contains(&self, panel_id: &str) -> bool {
        self.panels.iter().any(|p| p.panel_id == panel_id)
    }
}

#[derive(Default)]
pub struct FloatingRenderResult {
    pub panel_to_close: Option<String>,
    pub panel_to_dock: Option<String>,
    /// Grip dots were dragged — start a dock-drop drag (creates DragState in editor).
    pub redock_drag_started: Option<String>,
}

const MIN_FLOATING_SIZE: Vec2 = Vec2::new(200.0, 120.0);
const HEADER_HEIGHT: f32 = 28.0;
const CORNER_RADIUS: f32 = 8.0;
const EDGE_GRAB: f32 = 5.0;

pub fn render_floating_panels(
    ctx: &egui::Context,
    floating: &mut FloatingPanels,
    registry: &PanelRegistry,
    world: &World,
    theme: &Theme,
) -> FloatingRenderResult {
    let mut result = FloatingRenderResult::default();

    for panel in floating.panels.iter_mut() {
        let panel_def = registry.get(&panel.panel_id);
        let title = panel_def
            .map(|p| p.title().to_string())
            .unwrap_or_else(|| panel.panel_id.clone());
        let icon = panel_def.and_then(|p| p.icon().map(|s| s.to_string()));
        let min_size = panel_def
            .map(|p| {
                let m = p.min_size();
                Vec2::new(m[0].max(MIN_FLOATING_SIZE.x), m[1].max(MIN_FLOATING_SIZE.y))
            })
            .unwrap_or(MIN_FLOATING_SIZE);

        panel.size.x = panel.size.x.max(min_size.x);
        panel.size.y = panel.size.y.max(min_size.y);

        let base_id = Id::new("floating_panel").with(&panel.panel_id);

        render_single(
            ctx, panel, &title, icon.as_deref(), min_size,
            registry, world, theme, &mut result, base_id,
        );
    }

    result
}

/// Which edge/corner is being resized.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    fn cursor(self) -> CursorIcon {
        match self {
            Self::Left | Self::Right => CursorIcon::ResizeHorizontal,
            Self::Top | Self::Bottom => CursorIcon::ResizeVertical,
            Self::TopLeft | Self::BottomRight => CursorIcon::ResizeNwSe,
            Self::TopRight | Self::BottomLeft => CursorIcon::ResizeNeSw,
        }
    }

    fn apply(self, delta: Vec2, pos: &mut Pos2, size: &mut Vec2, min_size: Vec2) {
        match self {
            Self::Right => {
                size.x = (size.x + delta.x).max(min_size.x);
            }
            Self::Bottom => {
                size.y = (size.y + delta.y).max(min_size.y);
            }
            Self::Left => {
                let new_w = (size.x - delta.x).max(min_size.x);
                let actual_dx = size.x - new_w;
                pos.x += actual_dx;
                size.x = new_w;
            }
            Self::Top => {
                let new_h = (size.y - delta.y).max(min_size.y);
                let actual_dy = size.y - new_h;
                pos.y += actual_dy;
                size.y = new_h;
            }
            Self::TopLeft => {
                Self::Top.apply(Vec2::new(0.0, delta.y), pos, size, min_size);
                Self::Left.apply(Vec2::new(delta.x, 0.0), pos, size, min_size);
            }
            Self::TopRight => {
                Self::Top.apply(Vec2::new(0.0, delta.y), pos, size, min_size);
                Self::Right.apply(Vec2::new(delta.x, 0.0), pos, size, min_size);
            }
            Self::BottomLeft => {
                Self::Bottom.apply(Vec2::new(0.0, delta.y), pos, size, min_size);
                Self::Left.apply(Vec2::new(delta.x, 0.0), pos, size, min_size);
            }
            Self::BottomRight => {
                Self::Bottom.apply(Vec2::new(0.0, delta.y), pos, size, min_size);
                Self::Right.apply(Vec2::new(delta.x, 0.0), pos, size, min_size);
            }
        }
    }
}


fn render_single(
    ctx: &egui::Context,
    panel: &mut FloatingPanel,
    title: &str,
    icon: Option<&str>,
    min_size: Vec2,
    registry: &PanelRegistry,
    world: &World,
    theme: &Theme,
    result: &mut FloatingRenderResult,
    base_id: Id,
) {
    let panel_bg = theme.surfaces.panel.to_color32();
    let header_bg = theme.surfaces.extreme.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let rounding = egui::CornerRadius::same(CORNER_RADIUS as u8);
    let header_rounding = egui::CornerRadius {
        nw: CORNER_RADIUS as u8,
        ne: CORNER_RADIUS as u8,
        sw: 0,
        se: 0,
    };

    let total_rect = Rect::from_min_size(panel.pos, panel.size);
    let header_rect = Rect::from_min_size(panel.pos, Vec2::new(panel.size.x, HEADER_HEIGHT));
    let content_rect = Rect::from_min_max(
        Pos2::new(panel.pos.x, panel.pos.y + HEADER_HEIGHT),
        Pos2::new(panel.pos.x + panel.size.x, panel.pos.y + panel.size.y),
    );

    // ── Main Area for the panel body ──
    egui::Area::new(base_id)
        .fixed_pos(panel.pos)
        .order(Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            // Allocate expanded rect so edge resize zones are interactable
            ui.allocate_rect(total_rect.expand(EDGE_GRAB), Sense::hover());

            // ── Edge resize interactions (register first for priority on edges) ──
            let edge_rects: [(ResizeEdge, Rect); 8] = [
                // Corners first (highest priority)
                (ResizeEdge::TopLeft, Rect::from_min_max(
                    Pos2::new(total_rect.min.x - EDGE_GRAB, total_rect.min.y - EDGE_GRAB),
                    Pos2::new(total_rect.min.x + EDGE_GRAB, total_rect.min.y + EDGE_GRAB),
                )),
                (ResizeEdge::TopRight, Rect::from_min_max(
                    Pos2::new(total_rect.max.x - EDGE_GRAB, total_rect.min.y - EDGE_GRAB),
                    Pos2::new(total_rect.max.x + EDGE_GRAB, total_rect.min.y + EDGE_GRAB),
                )),
                (ResizeEdge::BottomLeft, Rect::from_min_max(
                    Pos2::new(total_rect.min.x - EDGE_GRAB, total_rect.max.y - EDGE_GRAB),
                    Pos2::new(total_rect.min.x + EDGE_GRAB, total_rect.max.y + EDGE_GRAB),
                )),
                (ResizeEdge::BottomRight, Rect::from_min_max(
                    Pos2::new(total_rect.max.x - EDGE_GRAB, total_rect.max.y - EDGE_GRAB),
                    Pos2::new(total_rect.max.x + EDGE_GRAB, total_rect.max.y + EDGE_GRAB),
                )),
                // Edges
                (ResizeEdge::Top, Rect::from_min_max(
                    Pos2::new(total_rect.min.x + EDGE_GRAB, total_rect.min.y - EDGE_GRAB),
                    Pos2::new(total_rect.max.x - EDGE_GRAB, total_rect.min.y + EDGE_GRAB),
                )),
                (ResizeEdge::Bottom, Rect::from_min_max(
                    Pos2::new(total_rect.min.x + EDGE_GRAB, total_rect.max.y - EDGE_GRAB),
                    Pos2::new(total_rect.max.x - EDGE_GRAB, total_rect.max.y + EDGE_GRAB),
                )),
                (ResizeEdge::Left, Rect::from_min_max(
                    Pos2::new(total_rect.min.x - EDGE_GRAB, total_rect.min.y + EDGE_GRAB),
                    Pos2::new(total_rect.min.x + EDGE_GRAB, total_rect.max.y - EDGE_GRAB),
                )),
                (ResizeEdge::Right, Rect::from_min_max(
                    Pos2::new(total_rect.max.x - EDGE_GRAB, total_rect.min.y + EDGE_GRAB),
                    Pos2::new(total_rect.max.x + EDGE_GRAB, total_rect.max.y - EDGE_GRAB),
                )),
            ];

            let mut any_edge_active = false;
            for (edge, rect) in &edge_rects {
                let resp = ui.interact(*rect, base_id.with(format!("edge_{:?}", edge)), Sense::drag());
                if resp.hovered() || resp.dragged() {
                    ui.ctx().set_cursor_icon(edge.cursor());
                    any_edge_active = true;
                }
                if resp.dragged() {
                    edge.apply(resp.drag_delta(), &mut panel.pos, &mut panel.size, min_size);
                }
            }

            // ── Header buttons & interactions (register small widgets first) ──

            // 1. Close button (rightmost)
            let close_center = Pos2::new(header_rect.max.x - 14.0, header_rect.center().y);
            let close_rect = Rect::from_center_size(close_center, Vec2::splat(16.0));
            let close_resp = ui.interact(close_rect, base_id.with("close"), Sense::click());
            if close_resp.clicked() {
                result.panel_to_close = Some(panel.panel_id.clone());
            }
            let close_hovered = close_resp.hovered();

            // 2. Dock button (left of close)
            let dock_center = Pos2::new(close_center.x - 22.0, header_rect.center().y);
            let dock_rect = Rect::from_center_size(dock_center, Vec2::splat(16.0));
            let dock_resp = ui.interact(dock_rect, base_id.with("dock_btn"), Sense::click());
            if dock_resp.clicked() {
                result.panel_to_dock = Some(panel.panel_id.clone());
            }
            let dock_hovered = dock_resp.hovered();

            // 3. Grip dots drag handle (leftmost area of header)
            let grip_rect = Rect::from_min_size(
                Pos2::new(header_rect.min.x + 4.0, header_rect.min.y + 4.0),
                Vec2::new(16.0, HEADER_HEIGHT - 8.0),
            );
            let grip_resp = ui.interact(grip_rect, base_id.with("grip"), Sense::click_and_drag());
            if grip_resp.drag_started() {
                result.redock_drag_started = Some(panel.panel_id.clone());
            }
            let grip_hovered = grip_resp.hovered();
            let grip_dragged = grip_resp.dragged();

            // 4. Header drag — the remaining area between grip and dock button
            let header_drag_rect = Rect::from_min_max(
                Pos2::new(grip_rect.max.x, header_rect.min.y),
                Pos2::new(dock_rect.min.x - 4.0, header_rect.max.y),
            );
            let header_resp = ui.interact(header_drag_rect, base_id.with("header"), Sense::click_and_drag());
            if header_resp.dragged() {
                panel.pos += header_resp.drag_delta();
            }

            // ── Cursors ──
            if close_hovered || dock_hovered {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            } else if grip_hovered || grip_dragged {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            } else if !any_edge_active && header_resp.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if !any_edge_active && header_resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }

            // ── Paint ──
            let painter = ui.painter();

            // Drop shadow
            let shadow_rect = total_rect.translate(Vec2::new(2.0, 4.0));
            for i in 0..3u8 {
                let expand = i as f32 * 2.0;
                let alpha = 40u8.saturating_sub(i * 12);
                painter.rect_filled(
                    shadow_rect.expand(expand),
                    egui::CornerRadius::same((CORNER_RADIUS + expand) as u8),
                    Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
                );
            }

            // Background + border
            painter.rect_filled(total_rect, rounding, panel_bg);
            painter.rect_stroke(total_rect, rounding, Stroke::new(1.0, border_color), egui::StrokeKind::Inside);

            // Header
            painter.rect_filled(header_rect, header_rounding, header_bg);
            painter.line_segment(
                [
                    Pos2::new(header_rect.min.x, header_rect.max.y),
                    Pos2::new(header_rect.max.x, header_rect.max.y),
                ],
                Stroke::new(1.0, border_color),
            );

            // Drag grip dots (2 cols x 3 rows) — highlight on hover
            let dot_color = if grip_hovered || grip_dragged {
                theme.text.secondary.to_color32()
            } else {
                theme.text.disabled.to_color32()
            };
            let dots_x = header_rect.min.x + 10.0;
            let dots_y = header_rect.center().y;
            for col in 0..2 {
                for row in 0..3 {
                    painter.circle_filled(
                        Pos2::new(dots_x + col as f32 * 4.0, dots_y - 4.0 + row as f32 * 4.0),
                        1.2,
                        dot_color,
                    );
                }
            }

            // Title
            let text_x = dots_x + 14.0;
            if let Some(icon_str) = icon {
                painter.text(
                    Pos2::new(text_x, header_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    icon_str,
                    egui::FontId::proportional(12.0),
                    theme.text.secondary.to_color32(),
                );
                painter.text(
                    Pos2::new(text_x + 16.0, header_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::FontId::proportional(11.0),
                    theme.text.primary.to_color32(),
                );
            } else {
                painter.text(
                    Pos2::new(text_x, header_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::FontId::proportional(11.0),
                    theme.text.primary.to_color32(),
                );
            }

            // Dock button visuals (rectangle with an arrow pointing in)
            if dock_hovered {
                let ac = theme.semantic.accent.to_color32();
                painter.rect_filled(
                    dock_rect.expand(2.0),
                    egui::CornerRadius::same(3),
                    Color32::from_rgba_unmultiplied(ac.r(), ac.g(), ac.b(), 30),
                );
            }
            let dock_color = if dock_hovered {
                theme.semantic.accent.to_color32()
            } else {
                theme.text.muted.to_color32()
            };
            // Draw a small docking icon: outer frame + inner rect
            let dc = dock_rect.center();
            let frame_rect = Rect::from_center_size(dc, Vec2::new(10.0, 8.0));
            painter.rect_stroke(frame_rect, egui::CornerRadius::same(1), Stroke::new(1.0, dock_color), egui::StrokeKind::Inside);
            // Inner filled portion (right half = "docked")
            let inner = Rect::from_min_max(
                Pos2::new(dc.x - 1.0, frame_rect.min.y + 1.5),
                Pos2::new(frame_rect.max.x - 1.5, frame_rect.max.y - 1.5),
            );
            painter.rect_filled(inner, egui::CornerRadius::ZERO, dock_color);

            // Close button visuals
            if close_hovered {
                painter.rect_filled(
                    close_rect.expand(2.0),
                    egui::CornerRadius::same(3),
                    Color32::from_rgba_unmultiplied(255, 80, 80, 40),
                );
            }
            let close_color = if close_hovered {
                theme.panels.close_hover.to_color32()
            } else {
                theme.text.muted.to_color32()
            };
            let cc = close_rect.center();
            let s = 3.5;
            painter.line_segment(
                [Pos2::new(cc.x - s, cc.y - s), Pos2::new(cc.x + s, cc.y + s)],
                Stroke::new(1.2, close_color),
            );
            painter.line_segment(
                [Pos2::new(cc.x + s, cc.y - s), Pos2::new(cc.x - s, cc.y + s)],
                Stroke::new(1.2, close_color),
            );

            // Resize grip dots (bottom-right)
            let grip_color = theme.text.disabled.to_color32();
            let bx = total_rect.max.x - 5.0;
            let by = total_rect.max.y - 5.0;
            for i in 0..3 {
                let offset = i as f32 * 4.0;
                painter.circle_filled(Pos2::new(bx - offset, by), 1.0, grip_color);
                if i > 0 {
                    painter.circle_filled(Pos2::new(bx, by - offset), 1.0, grip_color);
                }
            }
            painter.circle_filled(Pos2::new(bx - 4.0, by - 4.0), 1.0, grip_color);

            // ── Panel content ──
            let content_inset = content_rect.shrink(1.0);
            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(content_inset)
                    .id_salt(base_id.with("content")),
            );
            child_ui.set_clip_rect(content_inset);

            if let Some(p) = registry.get(&panel.panel_id) {
                p.ui(&mut child_ui, world);
            } else {
                child_ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new(format!("\"{}\" — not registered", panel.panel_id))
                            .color(theme.text.muted.to_color32()),
                    );
                });
            }

            // Right-click context menu on header
            let panel_id_clone = panel.panel_id.clone();
            header_resp.context_menu(|ui| {
                if ui.button("Dock").clicked() {
                    result.panel_to_dock = Some(panel_id_clone.clone());
                    ui.close();
                }
                if ui.button("Close").clicked() {
                    result.panel_to_close = Some(panel_id_clone.clone());
                    ui.close();
                }
            });
        });
}
