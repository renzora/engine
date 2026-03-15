//! Vertical tool overlay rendered in the top-left of the viewport (Blender-style).
//!
//! Always 2 columns. Top rows: Select | Move, Rotate | Scale.
//! Divider, then Undo | Redo. If terrain selected: another divider + terrain brush tools.
//! Play button sits in its own panel below.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use egui_phosphor::regular::*;

use std::sync::atomic::Ordering;

use renzora_core::PlayModeState;
use renzora_editor::{EditorCommands, EditorSelection, GizmoMode};
use renzora_terrain::data::*;

use crate::{NavOverlayState, AXIS_GIZMO_SIZE, AXIS_GIZMO_MARGIN};

const BTN_SIZE: Vec2 = Vec2::new(36.0, 36.0);
const BTN_GAP: f32 = 1.0;
const PADDING: f32 = 3.0;
const DIVIDER_GAP: f32 = 5.0;
const MARGIN: f32 = 8.0;

/// Render the vertical tool overlay on top of the viewport content area.
pub fn render_tool_overlay(ctx: &egui::Context, world: &World, content_rect: Rect) {
    let Some(theme_mgr) = world.get_resource::<renzora_theme::ThemeManager>() else { return };
    let theme = &theme_mgr.active_theme;
    let Some(cmds) = world.get_resource::<EditorCommands>() else { return };

    let play_mode = world.get_resource::<PlayModeState>();
    let is_playing = play_mode.map(|p| p.is_in_play_mode() || p.is_scripts_only()).unwrap_or(false);
    let hide_tools = is_playing;

    let row_step = BTN_SIZE.y + BTN_GAP;
    let panel_w = BTN_SIZE.x * 2.0 + BTN_GAP + PADDING * 2.0;
    let panel_pos = Pos2::new(content_rect.min.x + MARGIN, content_rect.min.y + MARGIN);

    let in_terrain = !hide_tools && is_terrain_selected(world);

    // Panel height: 2 gizmo rows + divider + 1 undo/redo row + optional terrain
    let panel_h = row_step * 3.0 - BTN_GAP + PADDING * 2.0 + DIVIDER_GAP * 2.0 + 1.0
        + if in_terrain {
            let terrain_rows = (terrain_tool_defs().len() + 1) / 2;
            DIVIDER_GAP * 2.0 + 1.0 + row_step * terrain_rows as f32
        } else {
            0.0
        };

    let panel_rect = Rect::from_min_size(panel_pos, Vec2::new(panel_w, panel_h));

    // Theme colors — legacy used darker bg
    let active_color = theme.semantic.accent.to_color32();
    let inactive_color = theme.widgets.inactive_bg.to_color32();
    let hovered_color = theme.widgets.hovered_bg.to_color32();
    let border_color = {
        let c = theme.widgets.border.to_color32();
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 120)
    };
    let panel_bg = {
        let c = theme.widgets.inactive_bg.to_color32();
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 255)
    };

    if !hide_tools {
        egui::Area::new(egui::Id::new("viewport_tool_overlay"))
            .fixed_pos(panel_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_clip_rect(panel_rect);

                ui.painter().rect_filled(panel_rect, CornerRadius::same(5), panel_bg);
                ui.painter().rect_stroke(panel_rect, CornerRadius::same(5), Stroke::new(1.0, border_color), egui::StrokeKind::Outside);

                let col0_x = panel_pos.x + PADDING;
                let col1_x = col0_x + BTN_SIZE.x + BTN_GAP;
                let mut y = panel_pos.y + PADDING;

                let gizmo_mode = world.get_resource::<GizmoMode>().copied().unwrap_or_default();
                let tool_state = world.get_resource::<TerrainToolState>();
                let is_sculpt_active = tool_state.map(|s| s.active).unwrap_or(false);

                // Row 0: Select | Move
                let is_select = !is_sculpt_active && gizmo_mode == GizmoMode::Select;
                let r = viewport_tool_button(ui, Rect::from_min_size(Pos2::new(col0_x, y), BTN_SIZE),
                    CURSOR, is_select, active_color, inactive_color, hovered_color);
                if r.clicked() {
                    cmds.push(move |w: &mut World| {
                        w.insert_resource(GizmoMode::Select);
                        if let Some(mut ts) = w.get_resource_mut::<TerrainToolState>() { ts.active = false; }
                    });
                }
                r.on_hover_text("Select (Q)");

                let is_translate = !is_sculpt_active && gizmo_mode == GizmoMode::Translate;
                let r = viewport_tool_button(ui, Rect::from_min_size(Pos2::new(col1_x, y), BTN_SIZE),
                    ARROWS_OUT_CARDINAL, is_translate, active_color, inactive_color, hovered_color);
                if r.clicked() {
                    cmds.push(move |w: &mut World| {
                        w.insert_resource(GizmoMode::Translate);
                        if let Some(mut ts) = w.get_resource_mut::<TerrainToolState>() { ts.active = false; }
                    });
                }
                r.on_hover_text("Move (W)");
                y += row_step;

                // Row 1: Rotate | Scale
                let is_rotate = !is_sculpt_active && gizmo_mode == GizmoMode::Rotate;
                let r = viewport_tool_button(ui, Rect::from_min_size(Pos2::new(col0_x, y), BTN_SIZE),
                    ARROWS_COUNTER_CLOCKWISE, is_rotate, active_color, inactive_color, hovered_color);
                if r.clicked() {
                    cmds.push(move |w: &mut World| {
                        w.insert_resource(GizmoMode::Rotate);
                        if let Some(mut ts) = w.get_resource_mut::<TerrainToolState>() { ts.active = false; }
                    });
                }
                r.on_hover_text("Rotate (E)");

                let is_scale = !is_sculpt_active && gizmo_mode == GizmoMode::Scale;
                let r = viewport_tool_button(ui, Rect::from_min_size(Pos2::new(col1_x, y), BTN_SIZE),
                    ARROWS_OUT_SIMPLE, is_scale, active_color, inactive_color, hovered_color);
                if r.clicked() {
                    cmds.push(move |w: &mut World| {
                        w.insert_resource(GizmoMode::Scale);
                        if let Some(mut ts) = w.get_resource_mut::<TerrainToolState>() { ts.active = false; }
                    });
                }
                r.on_hover_text("Scale (R)");
                y += row_step;

                // Divider before undo/redo
                draw_divider(ui, &mut y, panel_pos.x, panel_w, border_color);

                // Row 2: Undo | Redo (dimmed — no command history yet)
                let disabled_color = Color32::from_rgba_unmultiplied(inactive_color.r(), inactive_color.g(), inactive_color.b(), 80);
                let disabled_icon_color = Color32::from_white_alpha(40);

                let undo_rect = Rect::from_min_size(Pos2::new(col0_x, y), BTN_SIZE);
                ui.allocate_rect(undo_rect, Sense::hover());
                ui.painter().rect_filled(undo_rect, CornerRadius::same(3), disabled_color);
                ui.painter().text(undo_rect.center(), egui::Align2::CENTER_CENTER, ARROW_U_UP_LEFT, FontId::proportional(16.0), disabled_icon_color);
                ui.allocate_rect(undo_rect, Sense::hover()).on_hover_text("Undo (Ctrl+Z)");

                let redo_rect = Rect::from_min_size(Pos2::new(col1_x, y), BTN_SIZE);
                ui.allocate_rect(redo_rect, Sense::hover());
                ui.painter().rect_filled(redo_rect, CornerRadius::same(3), disabled_color);
                ui.painter().text(redo_rect.center(), egui::Align2::CENTER_CENTER, ARROW_U_UP_RIGHT, FontId::proportional(16.0), disabled_icon_color);
                ui.allocate_rect(redo_rect, Sense::hover()).on_hover_text("Redo (Ctrl+Y)");
                y += row_step;

                // Terrain brush tools
                if in_terrain {
                    draw_divider(ui, &mut y, panel_pos.x, panel_w, border_color);

                    let settings = world.get_resource::<TerrainSettings>();
                    let current_brush = settings.map(|s| s.brush_type).unwrap_or_default();
                    let terrain_tools = terrain_tool_defs();

                    for (i, (brush_type, icon, tip)) in terrain_tools.iter().enumerate() {
                        let bx = col0_x + (i % 2) as f32 * (BTN_SIZE.x + BTN_GAP);
                        let by = y + (i / 2) as f32 * row_step;
                        let is_active = is_sculpt_active && current_brush == *brush_type;

                        let r = viewport_tool_button(ui, Rect::from_min_size(Pos2::new(bx, by), BTN_SIZE),
                            icon, is_active, active_color, inactive_color, hovered_color);
                        if r.clicked() {
                            let bt = *brush_type;
                            if is_active {
                                cmds.push(move |w: &mut World| {
                                    if let Some(mut ts) = w.get_resource_mut::<TerrainToolState>() { ts.active = false; }
                                });
                            } else {
                                cmds.push(move |w: &mut World| {
                                    if let Some(mut ts) = w.get_resource_mut::<TerrainToolState>() { ts.active = true; }
                                    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() { s.brush_type = bt; }
                                });
                            }
                        }
                        r.on_hover_text(*tip);
                    }
                }
            });
    }

    // Play button — below tools in edit mode, bottom-center in play mode
    let play_panel_w = BTN_SIZE.x + PADDING * 2.0;
    let play_panel_h = BTN_SIZE.y + PADDING * 2.0;
    let play_panel_pos = if hide_tools {
        Pos2::new(
            content_rect.center().x - play_panel_w / 2.0,
            content_rect.max.y - play_panel_h - MARGIN,
        )
    } else {
        Pos2::new(panel_pos.x, panel_pos.y + panel_h + 4.0)
    };
    let play_panel_rect = Rect::from_min_size(play_panel_pos, Vec2::new(play_panel_w, play_panel_h));

    let is_in_play_mode = play_mode.map(|p| p.is_in_play_mode()).unwrap_or(false);
    let is_scripts_only = play_mode.map(|p| p.is_scripts_only()).unwrap_or(false);
    let play_color = theme.semantic.success.to_color32();
    let scripts_color = theme.semantic.accent.to_color32();
    let stop_color = theme.semantic.error.to_color32();

    egui::Area::new(egui::Id::new("viewport_play_overlay"))
        .fixed_pos(play_panel_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_clip_rect(play_panel_rect);

            ui.painter().rect_filled(play_panel_rect, CornerRadius::same(5), panel_bg);
            ui.painter().rect_stroke(play_panel_rect, CornerRadius::same(5), Stroke::new(1.0, border_color), egui::StrokeKind::Outside);

            let x_pos = play_panel_pos.x + PADDING;
            let y_pos = play_panel_pos.y + PADDING;
            let play_button_id = ui.make_persistent_id("viewport_play_dropdown");
            let play_btn_rect = Rect::from_min_size(Pos2::new(x_pos, y_pos), BTN_SIZE);
            let play_resp = ui.interact(play_btn_rect, play_button_id.with("btn"), Sense::click());
            if play_resp.hovered() { ui.ctx().set_cursor_icon(CursorIcon::PointingHand); }

            let (btn_icon, btn_bg) = if is_in_play_mode {
                (STOP, stop_color)
            } else if is_scripts_only {
                (STOP, scripts_color)
            } else if play_resp.hovered() {
                (PLAY, hovered_color)
            } else {
                (PLAY, inactive_color)
            };

            ui.painter().rect_filled(play_btn_rect, CornerRadius::same(3), btn_bg);
            let icon_color = if !is_in_play_mode && !is_scripts_only { play_color } else { Color32::WHITE };
            ui.painter().text(play_btn_rect.center(), egui::Align2::CENTER_CENTER, btn_icon, FontId::proportional(14.0), icon_color);

            if play_resp.clicked() {
                if is_in_play_mode || is_scripts_only {
                    cmds.push(|w: &mut World| {
                        if let Some(mut pm) = w.get_resource_mut::<PlayModeState>() { pm.request_stop = true; }
                    });
                } else {
                    #[allow(deprecated)]
                    ui.memory_mut(|mem| mem.toggle_popup(play_button_id));
                }
            }

            #[allow(deprecated)]
            egui::popup_below_widget(ui, play_button_id, &play_resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                ui.set_min_width(160.0);
                ui.style_mut().spacing.item_spacing.y = 2.0;
                if viewport_play_menu_item(ui, PLAY, "Play", "F5", play_color) {
                    cmds.push(|w: &mut World| {
                        if let Some(mut pm) = w.get_resource_mut::<PlayModeState>() { if pm.is_editing() { pm.request_play = true; } }
                    });
                    ui.close();
                }
                if viewport_play_menu_item(ui, CODE, "Run Scripts", "Shift+F5", scripts_color) {
                    cmds.push(|w: &mut World| {
                        if let Some(mut pm) = w.get_resource_mut::<PlayModeState>() { if pm.is_editing() { pm.request_scripts_only = true; } }
                    });
                    ui.close();
                }
            });
        });
}

// ── Tool Definitions ─────────────────────────────────────────────────────────

fn terrain_tool_defs() -> &'static [(TerrainBrushType, &'static str, &'static str)] {
    &[
        (TerrainBrushType::Sculpt,    MOUNTAINS,           "Sculpt"),
        (TerrainBrushType::Smooth,    WAVES,               "Smooth"),
        (TerrainBrushType::Flatten,   EQUALS,              "Flatten"),
        (TerrainBrushType::Raise,     ARROW_FAT_LINE_UP,   "Raise"),
        (TerrainBrushType::Erosion,   TREE,                "Erosion"),
        (TerrainBrushType::Hydro,     DROP,                "Hydro"),
        (TerrainBrushType::Noise,     WAVEFORM,            "Noise"),
        (TerrainBrushType::Retop,     GRAPH,               "Retop"),
        (TerrainBrushType::Terrace,   STAIRS,              "Terrace"),
        (TerrainBrushType::Pinch,     ARROWS_IN_CARDINAL,  "Pinch"),
        (TerrainBrushType::Erase,     ERASER,              "Erase"),
        (TerrainBrushType::Lower,     ARROW_FAT_LINE_DOWN, "Lower"),
    ]
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn draw_divider(ui: &mut egui::Ui, y: &mut f32, panel_x: f32, panel_w: f32, border_color: Color32) {
    *y += DIVIDER_GAP - BTN_GAP;
    let div_color = Color32::from_rgba_unmultiplied(border_color.r(), border_color.g(), border_color.b(), 80);
    let x0 = panel_x + PADDING + 4.0;
    let x1 = panel_x + panel_w - PADDING - 4.0;
    ui.painter().line_segment([Pos2::new(x0, *y), Pos2::new(x1, *y)], Stroke::new(1.0, div_color));
    *y += 1.0 + DIVIDER_GAP;
}

fn is_terrain_selected(world: &World) -> bool {
    let Some(sel) = world.get_resource::<EditorSelection>() else { return false };
    let Some(entity) = sel.get() else { return false };
    world.get::<TerrainData>(entity).is_some()
}

fn viewport_tool_button(
    ui: &mut egui::Ui, rect: Rect, icon: &str, active: bool,
    active_color: Color32, inactive_color: Color32, hovered_color: Color32,
) -> egui::Response {
    let resp = ui.allocate_rect(rect, Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(CursorIcon::PointingHand); }
    if ui.is_rect_visible(rect) {
        let bg = if active { active_color } else if resp.hovered() { hovered_color } else { inactive_color };
        ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon, FontId::proportional(16.0), Color32::WHITE);
    }
    resp
}

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

fn viewport_play_menu_item(ui: &mut egui::Ui, icon: &str, label: &str, shortcut: &str, icon_color: Color32) -> bool {
    let resp = ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).size(14.0).color(icon_color));
        ui.label(egui::RichText::new(label).size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(shortcut).size(10.0).color(Color32::from_gray(100)));
        });
    });
    resp.response.interact(Sense::click()).clicked()
}
