//! Vertical tool overlay rendered in the top-left of the viewport (Blender-style).
//!
//! Always 2 columns. Top rows: Select | Move, Rotate | Scale.
//! Divider, then Undo | Redo. If terrain selected: another divider + terrain brush tools.
//! Play button sits in its own panel below.

use bevy::prelude::*;
use renzora::bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use renzora::egui_phosphor::regular::*;

use std::sync::atomic::Ordering;

use renzora::core::PlayModeState;
use renzora::editor::{EditorCommands, ToolEntry, ToolSection, ToolbarRegistry};

use crate::{NavOverlayState, AXIS_GIZMO_SIZE, AXIS_GIZMO_MARGIN};

const BTN_SIZE: Vec2 = Vec2::new(32.0, 32.0);
const BTN_GAP: f32 = 1.0;
const PADDING: f32 = 3.0;
const DIVIDER_GAP: f32 = 4.0;
const MARGIN: f32 = 8.0;

/// Render the vertical tool overlay on top of the viewport content area.
pub fn render_tool_overlay(ctx: &egui::Context, world: &World, content_rect: Rect) {
    let Some(theme_mgr) = world.get_resource::<renzora::theme::ThemeManager>() else { return };
    let theme = &theme_mgr.active_theme;
    let Some(cmds) = world.get_resource::<EditorCommands>() else { return };

    let play_mode = world.get_resource::<PlayModeState>();
    let is_playing = play_mode.map(|p| p.is_in_play_mode() || p.is_scripts_only()).unwrap_or(false);
    let hide_tools = is_playing;

    let row_step = BTN_SIZE.y + BTN_GAP;
    let panel_w = BTN_SIZE.x + PADDING * 2.0;
    let panel_pos = Pos2::new(content_rect.min.x + MARGIN, content_rect.min.y + MARGIN);

    // Collect tool sections from the registry.
    let registry = world.get_resource::<ToolbarRegistry>();
    let transform_tools = registry
        .map(|r| r.visible_in_section(world, &ToolSection::Transform))
        .unwrap_or_default();
    let terrain_tools = registry
        .map(|r| r.visible_in_section(world, &ToolSection::Terrain))
        .unwrap_or_default();
    let custom_sections: Vec<(&'static str, Vec<ToolEntry>)> = registry
        .map(|r| {
            r.custom_sections()
                .into_iter()
                .map(|id| (id, r.visible_in_section(world, &ToolSection::Custom(id))))
                .filter(|(_, tools)| !tools.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Undo/Redo always shown when Transform section is visible.
    let has_undo_redo = !transform_tools.is_empty();

    let section_height = |count: usize| -> f32 {
        if count == 0 { return 0.0; }
        row_step * count as f32 - BTN_GAP
    };

    let transform_h = section_height(transform_tools.len());
    let undo_redo_h = if has_undo_redo { section_height(2) } else { 0.0 };
    let terrain_h = section_height(terrain_tools.len());
    let custom_h: f32 = custom_sections.iter().map(|(_, t)| section_height(t.len())).sum();
    let section_count = (transform_h > 0.0) as usize
        + (undo_redo_h > 0.0) as usize
        + (terrain_h > 0.0) as usize
        + custom_sections.len();
    let divider_count = section_count.saturating_sub(1);
    let dividers_h = (divider_count as f32) * (DIVIDER_GAP * 2.0 + 1.0);

    let has_tools = section_count > 0;
    let panel_h = PADDING * 2.0 + transform_h + undo_redo_h + terrain_h + custom_h + dividers_h;

    let panel_rect = Rect::from_min_size(panel_pos, Vec2::new(panel_w, panel_h));

    // Theme colors — legacy used darker bg
    let active_color = theme.semantic.accent.to_color32();
    let inactive_color = theme.widgets.inactive_bg.to_color32();
    let hovered_color = theme.widgets.hovered_bg.to_color32();
    let border_color = {
        let c = theme.widgets.border.to_color32();
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 120)
    };
    let panel_bg = theme.surfaces.panel.to_color32();

    if !hide_tools && has_tools {
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
                let mut first_section = true;

                // Transform section (Select / Translate / Rotate / Scale)
                if !transform_tools.is_empty() {
                    first_section = false;
                    render_tool_section(
                        ui, &transform_tools, cmds, world,
                        col0_x, col1_x, &mut y, row_step,
                        active_color, inactive_color, hovered_color,
                    );
                }

                // Undo / Redo
                if has_undo_redo {
                    if !first_section {
                        draw_divider(ui, &mut y, panel_pos.x, panel_w, border_color);
                    }
                    first_section = false;
                    let (can_undo, can_redo) = world.get_resource::<renzora::undo::UndoStacks>()
                        .map(|s| (s.can_undo(&s.active), s.can_redo(&s.active)))
                        .unwrap_or((false, false));
                    let undo_rect = Rect::from_min_size(Pos2::new(col0_x, y), BTN_SIZE);
                    let r = undo_redo_button(ui, undo_rect, ARROW_U_UP_LEFT, can_undo, inactive_color, hovered_color);
                    if can_undo && r.clicked() {
                        cmds.push(|w: &mut World| renzora::undo::undo_once(w));
                    }
                    r.on_hover_text("Undo (Ctrl+Z)");
                    y += row_step;
                    let redo_rect = Rect::from_min_size(Pos2::new(col0_x, y), BTN_SIZE);
                    let r = undo_redo_button(ui, redo_rect, ARROW_U_UP_RIGHT, can_redo, inactive_color, hovered_color);
                    if can_redo && r.clicked() {
                        cmds.push(|w: &mut World| renzora::undo::redo_once(w));
                    }
                    r.on_hover_text("Redo (Ctrl+Y)");
                    y += row_step;
                }

                // Terrain section
                if !terrain_tools.is_empty() {
                    if !first_section {
                        draw_divider(ui, &mut y, panel_pos.x, panel_w, border_color);
                    }
                    first_section = false;
                    render_tool_section(
                        ui, &terrain_tools, cmds, world,
                        col0_x, col1_x, &mut y, row_step,
                        active_color, inactive_color, hovered_color,
                    );
                }

                // Plugin-defined custom sections
                for (_id, tools) in &custom_sections {
                    if !first_section {
                        draw_divider(ui, &mut y, panel_pos.x, panel_w, border_color);
                    }
                    first_section = false;
                    render_tool_section(
                        ui, tools, cmds, world,
                        col0_x, col1_x, &mut y, row_step,
                        active_color, inactive_color, hovered_color,
                    );
                }
            });
    }

    // Play button — top-left in edit mode (tools are in the header), bottom-center in play.
    let play_panel_w = BTN_SIZE.x + PADDING * 2.0;
    let play_panel_h = BTN_SIZE.y + PADDING * 2.0;
    let play_panel_pos = if hide_tools {
        Pos2::new(
            content_rect.center().x - play_panel_w / 2.0,
            content_rect.max.y - play_panel_h - MARGIN,
        )
    } else if has_tools {
        Pos2::new(panel_pos.x, panel_pos.y + panel_h + 4.0)
    } else {
        Pos2::new(panel_pos.x, panel_pos.y)
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

// ── Helpers ──────────────────────────────────────────────────────────────────

fn draw_divider(ui: &mut egui::Ui, y: &mut f32, panel_x: f32, panel_w: f32, border_color: Color32) {
    *y += DIVIDER_GAP - BTN_GAP;
    let div_color = Color32::from_rgba_unmultiplied(border_color.r(), border_color.g(), border_color.b(), 80);
    let x0 = panel_x + PADDING + 4.0;
    let x1 = panel_x + panel_w - PADDING - 4.0;
    ui.painter().line_segment([Pos2::new(x0, *y), Pos2::new(x1, *y)], Stroke::new(1.0, div_color));
    *y += 1.0 + DIVIDER_GAP;
}

/// Render a toolbar section: lays entries out in a 2-column grid, top to
/// bottom. Each entry's predicates and activator are invoked from the registry.
fn render_tool_section(
    ui: &mut egui::Ui,
    tools: &[ToolEntry],
    cmds: &EditorCommands,
    world: &World,
    col0_x: f32,
    col1_x: f32,
    y: &mut f32,
    row_step: f32,
    active_color: Color32,
    inactive_color: Color32,
    hovered_color: Color32,
) {
    let _ = col1_x;
    for entry in tools.iter() {
        let rect = Rect::from_min_size(Pos2::new(col0_x, *y), BTN_SIZE);
        let is_active = (entry.is_active)(world);
        let r = viewport_tool_button(ui, rect, entry.icon, is_active, active_color, inactive_color, hovered_color);
        if r.clicked() {
            let activate = entry.activate.clone();
            cmds.push(move |w: &mut World| { (activate)(w); });
        }
        r.on_hover_text(entry.tooltip);
        *y += row_step;
    }
}

/// A toolbar button for undo/redo — enabled state accepts clicks; disabled
/// state renders dimmed and only shows a tooltip.
fn undo_redo_button(
    ui: &mut egui::Ui,
    rect: Rect,
    icon: &str,
    enabled: bool,
    _inactive_color: Color32,
    hovered_color: Color32,
) -> egui::Response {
    if !enabled {
        let resp = ui.allocate_rect(rect, Sense::hover());
        if ui.is_rect_visible(rect) {
            ui.painter().text(
                rect.center(), egui::Align2::CENTER_CENTER, icon,
                FontId::proportional(16.0), Color32::from_white_alpha(40),
            );
        }
        return resp;
    }
    let resp = ui.allocate_rect(rect, Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(CursorIcon::PointingHand); }
    if ui.is_rect_visible(rect) {
        if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(3), hovered_color);
        }
        ui.painter().text(
            rect.center(), egui::Align2::CENTER_CENTER, icon,
            FontId::proportional(16.0), Color32::WHITE,
        );
    }
    resp
}

fn viewport_tool_button(
    ui: &mut egui::Ui, rect: Rect, icon: &str, active: bool,
    active_color: Color32, _inactive_color: Color32, hovered_color: Color32,
) -> egui::Response {
    let resp = ui.allocate_rect(rect, Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(CursorIcon::PointingHand); }
    if ui.is_rect_visible(rect) {
        if active {
            ui.painter().rect_filled(rect, CornerRadius::same(3), active_color);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(3), hovered_color);
        }
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon, FontId::proportional(16.0), Color32::WHITE);
    }
    resp
}

/// Nav overlay: pan/zoom drag-buttons on the right side, below the axis gizmo.
pub fn render_nav_overlay(ctx: &egui::Context, world: &World, content_rect: Rect) {
    let Some(theme_mgr) = world.get_resource::<renzora::theme::ThemeManager>() else { return };
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
