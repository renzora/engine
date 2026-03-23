//! Viewport header bar — render toggles and dropdown menus.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Vec2};
use egui_phosphor::regular::*;
use renzora_editor::EditorCommands;
use renzora_theme::ThemeManager;

use crate::settings::*;

/// Height of the viewport header bar.
pub const HEADER_HEIGHT: f32 = 28.0;



/// View angle preset for the camera.
#[derive(Clone, Copy)]
enum ViewAngle {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

impl ViewAngle {
    const ALL: &'static [ViewAngle] = &[
        Self::Front, Self::Back, Self::Left, Self::Right, Self::Top, Self::Bottom,
    ];

    fn label(&self) -> &'static str {
        match self {
            Self::Front => "Front",  Self::Back => "Back",
            Self::Left => "Left",    Self::Right => "Right",
            Self::Top => "Top",      Self::Bottom => "Bottom",
        }
    }

    fn shortcut(&self) -> &'static str {
        match self {
            Self::Front => "Num1",      Self::Back => "Ctrl+Num1",
            Self::Left => "Ctrl+Num3",  Self::Right => "Num3",
            Self::Top => "Num7",        Self::Bottom => "Ctrl+Num7",
        }
    }

    fn yaw_pitch(&self) -> (f32, f32) {
        use std::f32::consts::{FRAC_PI_2, PI};
        match self {
            Self::Front => (0.0, 0.0),       Self::Back => (PI, 0.0),
            Self::Right => (FRAC_PI_2, 0.0), Self::Left => (-FRAC_PI_2, 0.0),
            Self::Top => (0.0, FRAC_PI_2),   Self::Bottom => (0.0, -FRAC_PI_2),
        }
    }
}

/// Render the viewport header bar with toggles and dropdown menus.
pub fn viewport_header(ui: &mut egui::Ui, world: &World) {
    let theme = match world.get_resource::<ThemeManager>() {
        Some(tm) => &tm.active_theme,
        None => return,
    };
    let cmds = world.get_resource::<EditorCommands>();
    let settings = world.get_resource::<ViewportSettings>();
    let (Some(settings), Some(cmds)) = (settings, cmds) else { return };

    let rect = ui.available_rect_before_wrap();
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEADER_HEIGHT));
    ui.advance_cursor_after_rect(header_rect);

    ui.painter().rect_filled(header_rect, 0.0, theme.surfaces.panel.to_color32());

    let active_color = theme.semantic.accent.to_color32();
    let inactive_color = theme.widgets.inactive_bg.to_color32();
    let hovered_color = theme.widgets.hovered_bg.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let icon_color = theme.text.secondary.to_color32();

    let btn_size = Vec2::new(28.0, 20.0);
    let drop_size = Vec2::new(36.0, 20.0);
    let btn_y = header_rect.min.y + (HEADER_HEIGHT - btn_size.y) / 2.0;

    let total_w = btn_size.x * 4.0 + 2.0 * 3.0 + 4.0 + drop_size.x * 5.0 + 4.0 * 4.0;
    let mut x = header_rect.center().x - total_w / 2.0;

    // === Render toggles ===
    let toggles = &settings.render_toggles;

    struct Toggle { icon: &'static str, active: bool, tip_on: &'static str, tip_off: &'static str, field: fn(&mut RenderToggles) -> &mut bool }
    let toggle_defs = [
        Toggle { icon: IMAGE,   active: toggles.textures,  tip_on: "Textures: ON",  tip_off: "Textures: OFF",  field: |t| &mut t.textures },
        Toggle { icon: POLYGON, active: toggles.wireframe, tip_on: "Wireframe: ON", tip_off: "Wireframe: OFF", field: |t| &mut t.wireframe },
        Toggle { icon: SUN,     active: toggles.lighting,  tip_on: "Lighting: ON",  tip_off: "Lighting: OFF",  field: |t| &mut t.lighting },
        Toggle { icon: CLOUD,   active: toggles.shadows,   tip_on: "Shadows: ON",   tip_off: "Shadows: OFF",   field: |t| &mut t.shadows },
    ];

    for (i, t) in toggle_defs.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, btn_y), btn_size);
        if tool_button(ui, r, t.icon, t.active, active_color, inactive_color, hovered_color) {
            let field_fn = t.field;
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    let val = field_fn(&mut s.render_toggles);
                    *val = !*val;
                }
            });
        }
        ui.allocate_rect(r, Sense::hover())
            .on_hover_text(if t.active { t.tip_on } else { t.tip_off });
        x += btn_size.x + if i < 3 { 2.0 } else { 4.0 };
    }

    // === Dropdown buttons ===
    let r = Rect::from_min_size(Pos2::new(x, btn_y), drop_size);
    gizmo_dropdown(ui, r, settings, cmds, icon_color, inactive_color, hovered_color, text_muted, theme);
    x += drop_size.x + 4.0;

    let r = Rect::from_min_size(Pos2::new(x, btn_y), drop_size);
    viz_dropdown(ui, r, settings, cmds, icon_color, inactive_color, hovered_color, text_muted);
    x += drop_size.x + 4.0;

    let r = Rect::from_min_size(Pos2::new(x, btn_y), drop_size);
    view_dropdown(ui, r, settings, cmds, icon_color, inactive_color, hovered_color, text_muted);
    x += drop_size.x + 4.0;

    let r = Rect::from_min_size(Pos2::new(x, btn_y), drop_size);
    camera_dropdown(ui, r, settings, cmds, icon_color, inactive_color, hovered_color, text_muted, theme);
    x += drop_size.x + 4.0;

    let r = Rect::from_min_size(Pos2::new(x, btn_y), drop_size);
    let any_snap = settings.snap.translate_enabled
        || settings.snap.rotate_enabled
        || settings.snap.scale_enabled
        || settings.snap.object_snap_enabled
        || settings.snap.floor_snap_enabled;
    let snap_bg = if any_snap { active_color } else { inactive_color };
    snap_dropdown(ui, r, settings, cmds, Color32::WHITE, snap_bg, hovered_color, text_muted, theme);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn tool_button(
    ui: &mut egui::Ui, rect: Rect, icon: &str, active: bool,
    active_color: Color32, inactive_color: Color32, hovered_color: Color32,
) -> bool {
    let resp = ui.allocate_rect(rect, Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    if ui.is_rect_visible(rect) {
        let bg = if active { active_color } else if resp.hovered() { hovered_color } else { inactive_color };
        ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon, FontId::proportional(16.0), Color32::WHITE);
    }
    resp.clicked()
}

fn dropdown_button(
    ui: &mut egui::Ui, rect: Rect, icon: &str,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
) -> egui::Response {
    let resp = ui.allocate_rect(rect, Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    if ui.is_rect_visible(rect) {
        let fill = if resp.hovered() { hovered_color } else { bg_color };
        ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
        ui.painter().text(Pos2::new(rect.left() + 10.0, rect.center().y), egui::Align2::CENTER_CENTER, icon, FontId::proportional(11.0), icon_color);
        ui.painter().text(Pos2::new(rect.right() - 8.0, rect.center().y), egui::Align2::CENTER_CENTER, CARET_DOWN, FontId::proportional(8.0), caret_color);
    }
    resp
}

// ── Gizmo/Overlays dropdown ─────────────────────────────────────────────────

fn gizmo_dropdown(
    ui: &mut egui::Ui, rect: Rect, settings: &ViewportSettings, cmds: &EditorCommands,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
    theme: &renzora_theme::Theme,
) {
    let id = ui.make_persistent_id("vp_gizmo_drop");
    let resp = dropdown_button(ui, rect, STACK, icon_color, bg_color, hovered_color, caret_color);
    if resp.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    let label_color = theme.text.muted.to_color32();
    let show_grid = settings.show_grid;
    let show_subgrid = settings.show_subgrid;
    let show_axis = settings.show_axis_gizmo;
    let collision_vis = settings.collision_gizmo_visibility;

    #[allow(deprecated)]
    egui::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(180.0);
        ui.style_mut().spacing.item_spacing.y = 4.0;

        ui.label(RichText::new("Overlays").small().color(label_color));
        ui.add_space(4.0);

        let mut grid = show_grid;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Grid").size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.checkbox(&mut grid, ""); });
        });
        if grid != show_grid {
            cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.show_grid = grid; } });
        }

        let mut subgrid = show_subgrid;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Sub-grid").size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled(show_grid, egui::Checkbox::new(&mut subgrid, ""));
            });
        });
        if subgrid != show_subgrid {
            cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.show_subgrid = subgrid; } });
        }

        let mut axis = show_axis;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Axis Gizmo").size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.checkbox(&mut axis, ""); });
        });
        if axis != show_axis {
            cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.show_axis_gizmo = axis; } });
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Collision Gizmos").small().color(label_color));
        ui.add_space(2.0);

        let mut selected_only = collision_vis == CollisionGizmoVisibility::SelectedOnly;
        if ui.radio_value(&mut selected_only, true, RichText::new("Selected Only").size(12.0)).clicked() {
            cmds.push(|w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.collision_gizmo_visibility = CollisionGizmoVisibility::SelectedOnly; } });
        }
        if ui.radio_value(&mut selected_only, false, RichText::new("Always").size(12.0)).clicked() {
            cmds.push(|w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.collision_gizmo_visibility = CollisionGizmoVisibility::Always; } });
        }
    });
}

// ── Visualization Mode dropdown ─────────────────────────────────────────────

fn viz_dropdown(
    ui: &mut egui::Ui, rect: Rect, settings: &ViewportSettings, cmds: &EditorCommands,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
) {
    let id = ui.make_persistent_id("vp_viz_drop");
    let resp = dropdown_button(ui, rect, EYE, icon_color, bg_color, hovered_color, caret_color);
    if resp.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    let resp = resp.on_hover_text("Visualization Mode");

    let current = settings.visualization_mode;

    #[allow(deprecated)]
    egui::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(120.0);
        ui.style_mut().spacing.item_spacing.y = 2.0;
        for mode in VisualizationMode::ALL {
            let is_selected = current == *mode;
            let label = if is_selected { format!("• {}", mode.label()) } else { format!("  {}", mode.label()) };
            if ui.add(
                egui::Button::new(&label).fill(Color32::TRANSPARENT).corner_radius(CornerRadius::same(2)).min_size(Vec2::new(ui.available_width(), 0.0)),
            ).clicked() {
                let m = *mode;
                cmds.push(move |w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.visualization_mode = m; } });
                ui.close();
            }
        }
    });
}

// ── View Angles dropdown ────────────────────────────────────────────────────

fn view_dropdown(
    ui: &mut egui::Ui, rect: Rect, settings: &ViewportSettings, cmds: &EditorCommands,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
) {
    let id = ui.make_persistent_id("vp_view_drop");
    let resp = dropdown_button(ui, rect, CUBE, icon_color, bg_color, hovered_color, caret_color);
    if resp.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    let resp = resp.on_hover_text("View Angle");

    let proj = settings.projection_mode;

    #[allow(deprecated)]
    egui::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(160.0);
        ui.style_mut().spacing.item_spacing.y = 2.0;

        ui.label(RichText::new("Projection").small().color(Color32::from_gray(140)));
        ui.add_space(2.0);

        let persp_label = if proj == ProjectionMode::Perspective { "• Perspective" } else { "  Perspective" };
        if ui.add(egui::Button::new(persp_label).fill(Color32::TRANSPARENT).corner_radius(CornerRadius::same(2)).min_size(Vec2::new(ui.available_width(), 0.0))).clicked() {
            cmds.push(|w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.projection_mode = ProjectionMode::Perspective; } });
        }

        let ortho_label = if proj == ProjectionMode::Orthographic { "• Orthographic" } else { "  Orthographic" };
        if ui.add(egui::Button::new(ortho_label).fill(Color32::TRANSPARENT).corner_radius(CornerRadius::same(2)).min_size(Vec2::new(ui.available_width(), 0.0))).clicked() {
            cmds.push(|w: &mut World| { if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.projection_mode = ProjectionMode::Orthographic; } });
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        ui.label(RichText::new("View Angles").small().color(Color32::from_gray(140)));
        ui.add_space(2.0);

        for view in ViewAngle::ALL {
            let label = format!("{}  ({})", view.label(), view.shortcut());
            if ui.add(egui::Button::new(&label).fill(Color32::TRANSPARENT).corner_radius(CornerRadius::same(2)).min_size(Vec2::new(ui.available_width(), 0.0))).clicked() {
                let (yaw, pitch) = view.yaw_pitch();
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                        s.pending_view_angle = Some(ViewAngleCommand { yaw, pitch });
                    }
                });
                ui.close();
            }
        }
    });
}

// ── Camera Settings dropdown ────────────────────────────────────────────────

fn camera_dropdown(
    ui: &mut egui::Ui, rect: Rect, settings: &ViewportSettings, cmds: &EditorCommands,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
    theme: &renzora_theme::Theme,
) {
    let id = ui.make_persistent_id("vp_camera_drop");
    let resp = dropdown_button(ui, rect, VIDEO_CAMERA, icon_color, bg_color, hovered_color, caret_color);
    if resp.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    let resp = resp.on_hover_text("Camera Settings");

    let label_color = theme.text.muted.to_color32();
    let mut cam = settings.camera;

    #[allow(deprecated)]
    egui::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(200.0);
        ui.style_mut().spacing.item_spacing.y = 4.0;

        ui.label(RichText::new("Camera Settings").small().color(label_color));
        ui.add_space(4.0);

        let drag_row = |ui: &mut egui::Ui, label_text: &str, val: &mut f32, range: std::ops::RangeInclusive<f64>, speed: f64, decimals: usize| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(label_text).size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::DragValue::new(val).range(range).speed(speed).max_decimals(decimals));
                });
            });
        };

        drag_row(ui, "Move Speed", &mut cam.move_speed, 0.1..=100.0, 0.5, 1);
        drag_row(ui, "Look Sensitivity", &mut cam.look_sensitivity, 0.05..=2.0, 0.05, 2);
        drag_row(ui, "Orbit Sensitivity", &mut cam.orbit_sensitivity, 0.05..=2.0, 0.05, 2);
        drag_row(ui, "Pan Sensitivity", &mut cam.pan_sensitivity, 0.1..=5.0, 0.1, 1);
        drag_row(ui, "Zoom Sensitivity", &mut cam.zoom_sensitivity, 0.1..=5.0, 0.1, 1);

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Invert Y Axis").size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.checkbox(&mut cam.invert_y, ""); });
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Distance Relative Speed").size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.checkbox(&mut cam.distance_relative_speed, ""); });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        if ui.add(egui::Button::new(RichText::new("Reset to Defaults").size(12.0)).min_size(Vec2::new(ui.available_width(), 24.0))).clicked() {
            cam = CameraSettingsState::default();
        }

        ui.add_space(4.0);
        ui.label(RichText::new("Controls:").small().color(label_color));
        ui.label(RichText::new("• RMB Drag: Look around").small().color(label_color));
        ui.label(RichText::new("• RMB + WASD: Fly mode").small().color(label_color));
        ui.label(RichText::new("• MMB Drag: Orbit").small().color(label_color));
        ui.label(RichText::new("• Alt + LMB: Orbit").small().color(label_color));
        ui.label(RichText::new("• Scroll: Zoom").small().color(label_color));

        cmds.push(move |w: &mut World| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.camera = cam; }
        });
    });
}

// ── Snap Settings dropdown ──────────────────────────────────────────────────

fn snap_dropdown(
    ui: &mut egui::Ui, rect: Rect, settings: &ViewportSettings, cmds: &EditorCommands,
    icon_color: Color32, bg_color: Color32, hovered_color: Color32, caret_color: Color32,
    theme: &renzora_theme::Theme,
) {
    let id = ui.make_persistent_id("vp_snap_drop");
    let resp = dropdown_button(ui, rect, MAGNET, icon_color, bg_color, hovered_color, caret_color);
    if resp.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    let resp = resp.on_hover_text("Snap Settings");

    let active_btn = theme.semantic.accent.to_color32();
    let inactive_btn = theme.widgets.inactive_bg.to_color32();
    let label_color = theme.text.muted.to_color32();
    let mut snap = settings.snap;

    #[allow(deprecated)]
    egui::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(180.0);
        ui.style_mut().spacing.item_spacing.y = 4.0;

        ui.label(RichText::new("Snapping").small().color(label_color));
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Position").size(12.0)).fill(if snap.translate_enabled { active_btn } else { inactive_btn }).min_size(Vec2::new(70.0, 20.0))).clicked() {
                snap.translate_enabled = !snap.translate_enabled;
            }
            ui.add(egui::DragValue::new(&mut snap.translate_snap).range(0.01..=100.0).speed(0.1).max_decimals(2));
        });

        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Rotation").size(12.0)).fill(if snap.rotate_enabled { active_btn } else { inactive_btn }).min_size(Vec2::new(70.0, 20.0))).clicked() {
                snap.rotate_enabled = !snap.rotate_enabled;
            }
            ui.add(egui::DragValue::new(&mut snap.rotate_snap).range(1.0..=90.0).speed(1.0).max_decimals(0).suffix("°"));
        });

        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Scale").size(12.0)).fill(if snap.scale_enabled { active_btn } else { inactive_btn }).min_size(Vec2::new(70.0, 20.0))).clicked() {
                snap.scale_enabled = !snap.scale_enabled;
            }
            ui.add(egui::DragValue::new(&mut snap.scale_snap).range(0.01..=10.0).speed(0.05).max_decimals(2));
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new("Object Snapping").small().color(label_color));
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Objects").size(12.0)).fill(if snap.object_snap_enabled { active_btn } else { inactive_btn }).min_size(Vec2::new(70.0, 20.0))).clicked() {
                snap.object_snap_enabled = !snap.object_snap_enabled;
            }
            ui.add(egui::DragValue::new(&mut snap.object_snap_distance).range(0.1..=10.0).speed(0.1).max_decimals(1).suffix(" dist"));
        });

        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Floor").size(12.0)).fill(if snap.floor_snap_enabled { active_btn } else { inactive_btn }).min_size(Vec2::new(70.0, 20.0))).clicked() {
                snap.floor_snap_enabled = !snap.floor_snap_enabled;
            }
            ui.add(egui::DragValue::new(&mut snap.floor_y).range(-1000.0..=1000.0).speed(0.1).max_decimals(1).suffix(" Y"));
        });

        cmds.push(move |w: &mut World| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() { s.snap = snap; }
        });
    });
}
