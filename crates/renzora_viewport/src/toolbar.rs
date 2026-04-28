//! Viewport overlays: nav (pan/zoom drag buttons on the right) lives here.
//! Tool buttons (Select/Translate/Rotate/Scale + terrain + custom sections)
//! moved into the header bar — see `header::render_left_tools`.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Sense, Vec2};
use egui_phosphor::regular::*;

use std::sync::atomic::Ordering;

use crate::{NavOverlayState, AXIS_GIZMO_SIZE, AXIS_GIZMO_MARGIN};

/// No-op stub — the viewport panel still calls this each frame, but tool
/// buttons now render inline in the header bar so there's nothing to draw.
pub fn render_tool_overlay(_ctx: &egui::Context, _world: &World, _content_rect: Rect) {}

/// Nav overlay: pan/zoom drag-buttons on the right side, below the axis gizmo.
pub fn render_nav_overlay(ctx: &egui::Context, world: &World, content_rect: Rect) {
    let Some(theme_mgr) = world.get_resource::<renzora_theme::ThemeManager>() else { return };
    let theme = &theme_mgr.active_theme;
    let Some(nav) = world.get_resource::<NavOverlayState>() else { return };

    let btn_size = Vec2::new(36.0, 36.0);
    let btn_gap = 1.0_f32;
    let padding = 3.0_f32;
    let panel_w = btn_size.x + padding * 2.0;
    let panel_h = btn_size.y * 2.0 + btn_gap + padding * 2.0;

    // Position: right edge, below the axis gizmo
    let gizmo_bottom_y = content_rect.min.y + AXIS_GIZMO_SIZE + AXIS_GIZMO_MARGIN;
    let panel_x = content_rect.max.x - panel_w - 8.0;
    let panel_y = gizmo_bottom_y + 24.0;
    let panel_pos = Pos2::new(panel_x, panel_y);
    let panel_rect = Rect::from_min_size(panel_pos, Vec2::new(panel_w, panel_h));

    let active_color = theme.semantic.accent.to_color32();
    let hovered_color = theme.widgets.hovered_bg.to_color32();
    let resting_color = {
        let c = theme.surfaces.panel.to_color32();
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 120)
    };

    let x_pos = panel_pos.x + padding;
    let pan_btn_rect = Rect::from_min_size(Pos2::new(x_pos, panel_pos.y + padding), btn_size);
    let zoom_btn_rect = Rect::from_min_size(
        Pos2::new(x_pos, panel_pos.y + padding + btn_size.y + btn_gap),
        btn_size,
    );

    egui::Area::new(egui::Id::new("viewport_nav_overlay"))
        .fixed_pos(panel_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_clip_rect(panel_rect);

            // Pan button — drag to pan
            let pan_resp = ui.interact(pan_btn_rect, egui::Id::new("nav_pan_btn"), Sense::drag());
            if pan_resp.drag_started() {
                nav.pan_dragging.store(true, Ordering::Relaxed);
                nav.zoom_dragging.store(false, Ordering::Relaxed);
            }
            if pan_resp.drag_stopped() {
                nav.pan_dragging.store(false, Ordering::Relaxed);
            }
            let pan_active = nav.pan_dragging.load(Ordering::Relaxed);
            if pan_resp.hovered() || pan_active {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }
            let pan_bg = if pan_active {
                active_color
            } else if pan_resp.hovered() {
                hovered_color
            } else {
                resting_color
            };
            let half_btn = (btn_size.x / 2.0) as u8;
            ui.painter().rect_filled(pan_btn_rect, CornerRadius::same(half_btn), pan_bg);
            ui.painter().text(
                pan_btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                HAND,
                FontId::proportional(16.0),
                Color32::WHITE,
            );

            // Zoom button — drag up/down to zoom
            let zoom_resp = ui.interact(zoom_btn_rect, egui::Id::new("nav_zoom_btn"), Sense::drag());
            if zoom_resp.drag_started() {
                nav.zoom_dragging.store(true, Ordering::Relaxed);
                nav.pan_dragging.store(false, Ordering::Relaxed);
            }
            if zoom_resp.drag_stopped() {
                nav.zoom_dragging.store(false, Ordering::Relaxed);
            }
            let zoom_active = nav.zoom_dragging.load(Ordering::Relaxed);
            if zoom_resp.hovered() || zoom_active {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
            }
            let zoom_bg = if zoom_active {
                active_color
            } else if zoom_resp.hovered() {
                hovered_color
            } else {
                resting_color
            };
            ui.painter().rect_filled(zoom_btn_rect, CornerRadius::same(half_btn), zoom_bg);
            ui.painter().text(
                zoom_btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                MAGNIFYING_GLASS,
                FontId::proportional(16.0),
                Color32::WHITE,
            );

            // Write drag deltas for the camera system to consume
            if pan_resp.dragged() {
                let d = pan_resp.drag_delta();
                nav.pan_delta_x.fetch_add((d.x * 1000.0) as i32, Ordering::Relaxed);
                nav.pan_delta_y.fetch_add((d.y * 1000.0) as i32, Ordering::Relaxed);
            }
            if zoom_resp.dragged() {
                let d = zoom_resp.drag_delta();
                nav.zoom_delta_y.fetch_add((d.y * 1000.0) as i32, Ordering::Relaxed);
            }
        });
}

