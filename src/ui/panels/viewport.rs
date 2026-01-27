use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, TextureId, Vec2};

use crate::core::{ViewportMode, ViewportState, AssetBrowserState, OrbitCameraState, GizmoState, PendingImageDrop, EditorSettings, VisualizationMode};
use crate::gizmo::{EditorTool, GizmoMode, ModalTransformState, AxisConstraint, SnapSettings};
use crate::viewport::Camera2DState;

use egui_phosphor::regular::{
    ARROWS_OUT_CARDINAL, ARROW_CLOCKWISE, ARROWS_OUT, CURSOR, MAGNET, CARET_DOWN,
    IMAGE, POLYGON, SUN, CLOUD, EYE,
};

/// Height of the viewport mode tabs bar
const VIEWPORT_TABS_HEIGHT: f32 = 24.0;
/// Width of rulers in 2D mode
const RULER_SIZE: f32 = 20.0;
/// Size of the axis gizmo
const AXIS_GIZMO_SIZE: f32 = 100.0;
/// Margin from viewport edge
const AXIS_GIZMO_MARGIN: f32 = 10.0;

pub fn render_viewport(
    ctx: &egui::Context,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    orbit: &mut OrbitCameraState,
    camera2d_state: &Camera2DState,
    gizmo: &mut GizmoState,
    modal_transform: &ModalTransformState,
    settings: &mut EditorSettings,
    left_panel_width: f32,
    right_panel_width: f32,
    content_start_y: f32,
    _window_size: [f32; 2],
    content_height: f32,
    viewport_texture_id: Option<TextureId>,
) {
    let screen_rect = ctx.screen_rect();

    // Calculate the full viewport area
    let full_viewport_rect = Rect::from_min_size(
        Pos2::new(left_panel_width, content_start_y),
        Vec2::new(
            screen_rect.width() - left_panel_width - right_panel_width,
            content_height,
        ),
    );

    // Render viewport mode tabs at the top (with tools)
    render_viewport_tabs(ctx, viewport, gizmo, settings, full_viewport_rect);

    // Calculate content rect (below tabs, accounting for rulers in 2D mode)
    let tabs_offset = VIEWPORT_TABS_HEIGHT;
    let ruler_offset = if viewport.viewport_mode == ViewportMode::Mode2D { RULER_SIZE } else { 0.0 };

    let content_rect = Rect::from_min_size(
        Pos2::new(full_viewport_rect.min.x + ruler_offset, full_viewport_rect.min.y + tabs_offset + ruler_offset),
        Vec2::new(
            full_viewport_rect.width() - ruler_offset,
            full_viewport_rect.height() - tabs_offset - ruler_offset,
        ),
    );

    // Render rulers in 2D mode
    if viewport.viewport_mode == ViewportMode::Mode2D {
        render_rulers(ctx, viewport, camera2d_state, full_viewport_rect, tabs_offset);
    }

    // Use an Area to render the viewport content
    // Use Order::Middle so it can receive input (positioned below tab bar so doesn't overlap)
    egui::Area::new(egui::Id::new("viewport_area"))
        .fixed_pos(content_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(content_rect);
            render_viewport_content(ui, viewport, assets, orbit, viewport_texture_id, content_rect);
        });

    // Render box selection rectangle (if active)
    if gizmo.box_selection.active && gizmo.box_selection.is_drag() {
        render_box_selection(ctx, gizmo);
    }

    // Render modal transform HUD (if active)
    if modal_transform.active {
        render_modal_transform_hud(ctx, modal_transform, content_rect);
    }
}

/// Render the 3D/2D mode tabs at the top of the viewport with tool controls
fn render_viewport_tabs(
    ctx: &egui::Context,
    viewport: &mut ViewportState,
    gizmo: &mut GizmoState,
    settings: &mut EditorSettings,
    full_rect: Rect,
) {
    let tabs_rect = Rect::from_min_size(
        full_rect.min,
        Vec2::new(full_rect.width(), VIEWPORT_TABS_HEIGHT),
    );

    let active_color = Color32::from_rgb(66, 150, 250);
    let inactive_color = Color32::from_rgb(46, 46, 56);
    let button_size = Vec2::new(28.0, 20.0);
    let button_y = tabs_rect.min.y + (VIEWPORT_TABS_HEIGHT - button_size.y) / 2.0;

    egui::Area::new(egui::Id::new("viewport_tabs"))
        .fixed_pos(tabs_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(tabs_rect);

            // Background
            ui.painter().rect_filled(tabs_rect, 0.0, Color32::from_rgb(35, 35, 40));

            let tab_height = VIEWPORT_TABS_HEIGHT - 4.0;
            let tab_y = tabs_rect.min.y + 2.0;
            let mut x_pos = tabs_rect.min.x + 4.0;

            // === 3D/2D Mode Tabs ===
            let tab_width = 36.0;

            // 3D tab
            let tab_3d_rect = Rect::from_min_size(
                Pos2::new(x_pos, tab_y),
                Vec2::new(tab_width, tab_height),
            );
            let is_3d_active = viewport.viewport_mode == ViewportMode::Mode3D;
            let tab_3d_color = if is_3d_active {
                Color32::from_rgb(60, 60, 70)
            } else {
                Color32::from_rgb(45, 45, 50)
            };
            ui.painter().rect_filled(tab_3d_rect, 3.0, tab_3d_color);
            ui.painter().text(
                tab_3d_rect.center(),
                egui::Align2::CENTER_CENTER,
                "3D",
                FontId::proportional(11.0),
                if is_3d_active { Color32::WHITE } else { Color32::from_rgb(150, 150, 160) },
            );
            x_pos += tab_width + 2.0;

            // 2D tab
            let tab_2d_rect = Rect::from_min_size(
                Pos2::new(x_pos, tab_y),
                Vec2::new(tab_width, tab_height),
            );
            let is_2d_active = viewport.viewport_mode == ViewportMode::Mode2D;
            let tab_2d_color = if is_2d_active {
                Color32::from_rgb(60, 60, 70)
            } else {
                Color32::from_rgb(45, 45, 50)
            };
            ui.painter().rect_filled(tab_2d_rect, 3.0, tab_2d_color);
            ui.painter().text(
                tab_2d_rect.center(),
                egui::Align2::CENTER_CENTER,
                "2D",
                FontId::proportional(11.0),
                if is_2d_active { Color32::WHITE } else { Color32::from_rgb(150, 150, 160) },
            );
            x_pos += tab_width + 8.0;

            // Separator
            ui.painter().line_segment(
                [Pos2::new(x_pos, tab_y + 3.0), Pos2::new(x_pos, tab_y + tab_height - 3.0)],
                Stroke::new(1.0, Color32::from_rgb(60, 60, 70)),
            );
            x_pos += 8.0;

            // === Selection Tool ===
            let is_select = gizmo.tool == EditorTool::Select;
            let select_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let select_resp = viewport_tool_button(ui, select_rect, CURSOR, is_select, active_color, inactive_color);
            if select_resp.clicked() {
                gizmo.tool = EditorTool::Select;
            }
            select_resp.on_hover_text("Select (Q)");
            x_pos += button_size.x + 2.0;

            // === Transform Tools ===
            let is_translate = gizmo.tool == EditorTool::Transform && gizmo.mode == GizmoMode::Translate;
            let translate_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let translate_resp = viewport_tool_button(ui, translate_rect, ARROWS_OUT_CARDINAL, is_translate, active_color, inactive_color);
            if translate_resp.clicked() {
                gizmo.tool = EditorTool::Transform;
                gizmo.mode = GizmoMode::Translate;
            }
            translate_resp.on_hover_text("Move (W)");
            x_pos += button_size.x + 2.0;

            let is_rotate = gizmo.tool == EditorTool::Transform && gizmo.mode == GizmoMode::Rotate;
            let rotate_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let rotate_resp = viewport_tool_button(ui, rotate_rect, ARROW_CLOCKWISE, is_rotate, active_color, inactive_color);
            if rotate_resp.clicked() {
                gizmo.tool = EditorTool::Transform;
                gizmo.mode = GizmoMode::Rotate;
            }
            rotate_resp.on_hover_text("Rotate (E)");
            x_pos += button_size.x + 2.0;

            let is_scale = gizmo.tool == EditorTool::Transform && gizmo.mode == GizmoMode::Scale;
            let scale_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let scale_resp = viewport_tool_button(ui, scale_rect, ARROWS_OUT, is_scale, active_color, inactive_color);
            if scale_resp.clicked() {
                gizmo.tool = EditorTool::Transform;
                gizmo.mode = GizmoMode::Scale;
            }
            scale_resp.on_hover_text("Scale (R)");
            x_pos += button_size.x + 4.0;

            // === Snap Dropdown ===
            let any_snap_enabled = gizmo.snap.translate_enabled || gizmo.snap.rotate_enabled || gizmo.snap.scale_enabled;
            let snap_color = if any_snap_enabled {
                Color32::from_rgb(140, 191, 242)
            } else {
                Color32::from_rgb(140, 140, 150)
            };
            let snap_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), Vec2::new(36.0, button_size.y));
            viewport_snap_dropdown(ui, ctx, snap_rect, MAGNET, snap_color, inactive_color, &mut gizmo.snap);
            x_pos += 36.0 + 8.0;

            // Separator
            ui.painter().line_segment(
                [Pos2::new(x_pos, tab_y + 3.0), Pos2::new(x_pos, tab_y + tab_height - 3.0)],
                Stroke::new(1.0, Color32::from_rgb(60, 60, 70)),
            );
            x_pos += 8.0;

            // === Render Toggles ===
            let toggle_on_color = Color32::from_rgb(66, 150, 250);
            let toggle_off_color = Color32::from_rgb(60, 60, 70);

            // Textures toggle
            let tex_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let tex_resp = viewport_tool_button(ui, tex_rect, IMAGE, settings.render_toggles.textures, toggle_on_color, toggle_off_color);
            if tex_resp.clicked() {
                settings.render_toggles.textures = !settings.render_toggles.textures;
            }
            tex_resp.on_hover_text(if settings.render_toggles.textures { "Textures: ON" } else { "Textures: OFF" });
            x_pos += button_size.x + 2.0;

            // Wireframe toggle
            let wire_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let wire_resp = viewport_tool_button(ui, wire_rect, POLYGON, settings.render_toggles.wireframe, toggle_on_color, toggle_off_color);
            if wire_resp.clicked() {
                settings.render_toggles.wireframe = !settings.render_toggles.wireframe;
            }
            wire_resp.on_hover_text(if settings.render_toggles.wireframe { "Wireframe: ON" } else { "Wireframe: OFF" });
            x_pos += button_size.x + 2.0;

            // Lighting toggle
            let light_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let light_resp = viewport_tool_button(ui, light_rect, SUN, settings.render_toggles.lighting, toggle_on_color, toggle_off_color);
            if light_resp.clicked() {
                settings.render_toggles.lighting = !settings.render_toggles.lighting;
            }
            light_resp.on_hover_text(if settings.render_toggles.lighting { "Lighting: ON" } else { "Lighting: OFF" });
            x_pos += button_size.x + 2.0;

            // Shadows toggle
            let shadow_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let shadow_resp = viewport_tool_button(ui, shadow_rect, CLOUD, settings.render_toggles.shadows, toggle_on_color, toggle_off_color);
            if shadow_resp.clicked() {
                settings.render_toggles.shadows = !settings.render_toggles.shadows;
            }
            shadow_resp.on_hover_text(if settings.render_toggles.shadows { "Shadows: ON" } else { "Shadows: OFF" });
            x_pos += button_size.x + 4.0;

            // === Visualization Mode Dropdown ===
            let viz_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), Vec2::new(36.0, button_size.y));
            let viz_color = Color32::from_rgb(180, 180, 200);
            viewport_viz_dropdown(ui, ctx, viz_rect, EYE, viz_color, inactive_color, settings);

            // Handle 3D/2D tab clicks
            if ctx.input(|i| i.pointer.any_click()) {
                if let Some(pos) = ctx.pointer_hover_pos() {
                    if tab_3d_rect.contains(pos) {
                        viewport.viewport_mode = ViewportMode::Mode3D;
                    } else if tab_2d_rect.contains(pos) {
                        viewport.viewport_mode = ViewportMode::Mode2D;
                    }
                }
            }
        });
}

/// Tool button for viewport header
fn viewport_tool_button(
    ui: &mut egui::Ui,
    rect: Rect,
    icon: &str,
    active: bool,
    active_color: Color32,
    inactive_color: Color32,
) -> egui::Response {
    let response = ui.allocate_rect(rect, Sense::click());

    if ui.is_rect_visible(rect) {
        let bg_color = if active {
            active_color
        } else if response.hovered() {
            Color32::from_rgb(56, 56, 68)
        } else {
            inactive_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(3), bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            FontId::proportional(13.0),
            Color32::WHITE,
        );
    }

    response
}

/// Snap dropdown for viewport header
fn viewport_snap_dropdown(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    rect: Rect,
    icon: &str,
    icon_color: Color32,
    bg_color: Color32,
    snap: &mut SnapSettings,
) {
    let button_id = ui.make_persistent_id("viewport_snap_dropdown");
    let response = ui.allocate_rect(rect, Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            Color32::from_rgb(56, 56, 68)
        } else {
            bg_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(3), fill);

        // Icon
        ui.painter().text(
            Pos2::new(rect.left() + 10.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            FontId::proportional(11.0),
            icon_color,
        );

        // Caret
        ui.painter().text(
            Pos2::new(rect.right() - 8.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            CARET_DOWN,
            FontId::proportional(8.0),
            Color32::from_rgb(140, 140, 150),
        );
    }

    if response.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(button_id));
    }

    #[allow(deprecated)]
    egui::popup_below_widget(
        ui,
        button_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(180.0);
            ui.style_mut().spacing.item_spacing.y = 4.0;

            ui.label(RichText::new("Snapping").small().color(Color32::from_rgb(140, 140, 150)));
            ui.add_space(4.0);

            // Position snap
            ui.horizontal(|ui| {
                let pos_active = snap.translate_enabled;
                if ui.add(
                    egui::Button::new(RichText::new("Position").size(12.0))
                        .fill(if pos_active { Color32::from_rgb(51, 85, 115) } else { Color32::from_rgb(45, 47, 53) })
                        .min_size(Vec2::new(70.0, 20.0))
                ).clicked() {
                    snap.translate_enabled = !snap.translate_enabled;
                }

                ui.add(
                    egui::DragValue::new(&mut snap.translate_snap)
                        .range(0.01..=100.0)
                        .speed(0.1)
                        .max_decimals(2)
                );
            });

            // Rotation snap
            ui.horizontal(|ui| {
                let rot_active = snap.rotate_enabled;
                if ui.add(
                    egui::Button::new(RichText::new("Rotation").size(12.0))
                        .fill(if rot_active { Color32::from_rgb(51, 85, 115) } else { Color32::from_rgb(45, 47, 53) })
                        .min_size(Vec2::new(70.0, 20.0))
                ).clicked() {
                    snap.rotate_enabled = !snap.rotate_enabled;
                }

                ui.add(
                    egui::DragValue::new(&mut snap.rotate_snap)
                        .range(1.0..=90.0)
                        .speed(1.0)
                        .max_decimals(0)
                        .suffix("°")
                );
            });

            // Scale snap
            ui.horizontal(|ui| {
                let scale_active = snap.scale_enabled;
                if ui.add(
                    egui::Button::new(RichText::new("Scale").size(12.0))
                        .fill(if scale_active { Color32::from_rgb(51, 85, 115) } else { Color32::from_rgb(45, 47, 53) })
                        .min_size(Vec2::new(70.0, 20.0))
                ).clicked() {
                    snap.scale_enabled = !snap.scale_enabled;
                }

                ui.add(
                    egui::DragValue::new(&mut snap.scale_snap)
                        .range(0.01..=10.0)
                        .speed(0.05)
                        .max_decimals(2)
                );
            });
        },
    );

    response.on_hover_text("Snap Settings");
}

/// Visualization mode dropdown for viewport header
fn viewport_viz_dropdown(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    rect: Rect,
    icon: &str,
    icon_color: Color32,
    bg_color: Color32,
    settings: &mut EditorSettings,
) {
    let button_id = ui.make_persistent_id("viewport_viz_dropdown");
    let response = ui.allocate_rect(rect, Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            Color32::from_rgb(56, 56, 68)
        } else {
            bg_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(3), fill);

        // Icon
        ui.painter().text(
            Pos2::new(rect.left() + 10.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            FontId::proportional(11.0),
            icon_color,
        );

        // Caret
        ui.painter().text(
            Pos2::new(rect.right() - 8.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            CARET_DOWN,
            FontId::proportional(8.0),
            Color32::from_rgb(140, 140, 150),
        );
    }

    if response.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(button_id));
    }

    #[allow(deprecated)]
    egui::popup_below_widget(
        ui,
        button_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(120.0);
            ui.style_mut().spacing.item_spacing.y = 2.0;

            for mode in VisualizationMode::ALL {
                let is_selected = settings.visualization_mode == *mode;
                let label = if is_selected {
                    format!("• {}", mode.label())
                } else {
                    format!("  {}", mode.label())
                };
                if ui.add(
                    egui::Button::new(&label)
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(CornerRadius::same(2))
                        .min_size(Vec2::new(ui.available_width(), 0.0))
                ).clicked() {
                    settings.visualization_mode = *mode;
                    ui.close();
                }
            }
        },
    );

    response.on_hover_text("Visualization Mode");
}

/// Render rulers for 2D mode
fn render_rulers(
    ctx: &egui::Context,
    viewport: &ViewportState,
    camera2d_state: &Camera2DState,
    full_rect: Rect,
    tabs_offset: f32,
) {
    let ruler_bg = Color32::from_rgb(40, 40, 45);
    let ruler_tick = Color32::from_rgb(100, 100, 110);
    let ruler_text = Color32::from_rgb(140, 140, 150);
    let ruler_major_tick = Color32::from_rgb(150, 150, 160);

    // Horizontal ruler (top)
    let h_ruler_rect = Rect::from_min_size(
        Pos2::new(full_rect.min.x + RULER_SIZE, full_rect.min.y + tabs_offset),
        Vec2::new(full_rect.width() - RULER_SIZE, RULER_SIZE),
    );

    egui::Area::new(egui::Id::new("h_ruler"))
        .fixed_pos(h_ruler_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(h_ruler_rect);
            ui.painter().rect_filled(h_ruler_rect, 0.0, ruler_bg);

            // Calculate tick spacing based on zoom
            let tick_spacing = calculate_ruler_tick_spacing(camera2d_state.zoom);
            let world_left = camera2d_state.pan_offset.x - (viewport.size[0] / camera2d_state.zoom / 2.0);
            let world_right = camera2d_state.pan_offset.x + (viewport.size[0] / camera2d_state.zoom / 2.0);

            // Draw tick marks
            let start_tick = (world_left / tick_spacing).floor() as i32;
            let end_tick = (world_right / tick_spacing).ceil() as i32;

            for i in start_tick..=end_tick {
                let world_x = i as f32 * tick_spacing;
                let screen_x = world_to_screen_x(world_x, camera2d_state, viewport);
                let local_x = screen_x - full_rect.min.x;

                if local_x < RULER_SIZE || local_x > full_rect.width() {
                    continue;
                }

                let is_major = i % 10 == 0;
                let tick_height = if is_major { RULER_SIZE * 0.6 } else { RULER_SIZE * 0.3 };
                let tick_y = h_ruler_rect.max.y - tick_height;

                ui.painter().line_segment(
                    [Pos2::new(screen_x, tick_y), Pos2::new(screen_x, h_ruler_rect.max.y)],
                    Stroke::new(1.0, if is_major { ruler_major_tick } else { ruler_tick }),
                );

                // Draw label for major ticks
                if is_major {
                    ui.painter().text(
                        Pos2::new(screen_x + 2.0, h_ruler_rect.min.y + 2.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", world_x as i32),
                        FontId::proportional(9.0),
                        ruler_text,
                    );
                }
            }
        });

    // Vertical ruler (left)
    let v_ruler_rect = Rect::from_min_size(
        Pos2::new(full_rect.min.x, full_rect.min.y + tabs_offset + RULER_SIZE),
        Vec2::new(RULER_SIZE, full_rect.height() - tabs_offset - RULER_SIZE),
    );

    egui::Area::new(egui::Id::new("v_ruler"))
        .fixed_pos(v_ruler_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(v_ruler_rect);
            ui.painter().rect_filled(v_ruler_rect, 0.0, ruler_bg);

            // Calculate tick spacing based on zoom
            let tick_spacing = calculate_ruler_tick_spacing(camera2d_state.zoom);
            let world_bottom = camera2d_state.pan_offset.y - (viewport.size[1] / camera2d_state.zoom / 2.0);
            let world_top = camera2d_state.pan_offset.y + (viewport.size[1] / camera2d_state.zoom / 2.0);

            // Draw tick marks
            let start_tick = (world_bottom / tick_spacing).floor() as i32;
            let end_tick = (world_top / tick_spacing).ceil() as i32;

            for i in start_tick..=end_tick {
                let world_y = i as f32 * tick_spacing;
                let screen_y = world_to_screen_y(world_y, camera2d_state, viewport);
                let local_y = screen_y - full_rect.min.y - tabs_offset;

                if local_y < RULER_SIZE || local_y > full_rect.height() - tabs_offset {
                    continue;
                }

                let is_major = i % 10 == 0;
                let tick_width = if is_major { RULER_SIZE * 0.6 } else { RULER_SIZE * 0.3 };
                let tick_x = v_ruler_rect.max.x - tick_width;

                ui.painter().line_segment(
                    [Pos2::new(tick_x, screen_y), Pos2::new(v_ruler_rect.max.x, screen_y)],
                    Stroke::new(1.0, if is_major { ruler_major_tick } else { ruler_tick }),
                );

                // Draw label for major ticks
                if is_major {
                    ui.painter().text(
                        Pos2::new(v_ruler_rect.min.x + 2.0, screen_y - 8.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", world_y as i32),
                        FontId::proportional(9.0),
                        ruler_text,
                    );
                }
            }
        });

    // Corner square (top-left)
    let corner_rect = Rect::from_min_size(
        Pos2::new(full_rect.min.x, full_rect.min.y + tabs_offset),
        Vec2::new(RULER_SIZE, RULER_SIZE),
    );
    egui::Area::new(egui::Id::new("ruler_corner"))
        .fixed_pos(corner_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.painter().rect_filled(corner_rect, 0.0, ruler_bg);
        });
}

/// Calculate tick spacing for rulers based on zoom level
fn calculate_ruler_tick_spacing(zoom: f32) -> f32 {
    // Target spacing in screen pixels
    let target_spacing = 50.0;
    let ideal_world_spacing = target_spacing / zoom;

    // Round to a nice number
    let nice_numbers = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
    let mut best = nice_numbers[0];
    let mut best_diff = (ideal_world_spacing - best).abs();

    for &nice in &nice_numbers {
        let diff = (ideal_world_spacing - nice).abs();
        if diff < best_diff {
            best_diff = diff;
            best = nice;
        }
    }
    best
}

/// Convert world X coordinate to screen X coordinate
fn world_to_screen_x(world_x: f32, camera2d_state: &Camera2DState, viewport: &ViewportState) -> f32 {
    let relative_x = world_x - camera2d_state.pan_offset.x;
    let screen_relative = relative_x * camera2d_state.zoom;
    viewport.position[0] + RULER_SIZE + viewport.size[0] / 2.0 + screen_relative
}

/// Convert world Y coordinate to screen Y coordinate
fn world_to_screen_y(world_y: f32, camera2d_state: &Camera2DState, viewport: &ViewportState) -> f32 {
    let relative_y = world_y - camera2d_state.pan_offset.y;
    let screen_relative = -relative_y * camera2d_state.zoom; // Y is inverted in screen coords
    viewport.position[1] + VIEWPORT_TABS_HEIGHT + RULER_SIZE + viewport.size[1] / 2.0 + screen_relative
}

/// Render viewport content (for use in docking)
pub fn render_viewport_content(
    ui: &mut egui::Ui,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    orbit: &mut OrbitCameraState,
    viewport_texture_id: Option<TextureId>,
    content_rect: Rect,
) {
    let ctx = ui.ctx().clone();

    // Update viewport state with the actual content area
    viewport.position = [content_rect.min.x, content_rect.min.y];
    viewport.size = [content_rect.width(), content_rect.height()];
    viewport.hovered = ui.rect_contains_pointer(content_rect);

    // Display the viewport texture if available
    if let Some(texture_id) = viewport_texture_id {
        // Allocate the space
        let (_rect, _response) = ui.allocate_exact_size(
            Vec2::new(content_rect.width(), content_rect.height()),
            egui::Sense::hover(),
        );

        // Draw the image
        let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
        ui.painter().image(texture_id, content_rect, uv, Color32::WHITE);
    } else {
        // No texture yet - draw placeholder
        ui.painter().rect_filled(content_rect, 0.0, Color32::from_rgb(30, 30, 35));

        // Draw "No Viewport" text centered
        ui.painter().text(
            content_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Viewport Loading...",
            egui::FontId::proportional(14.0),
            Color32::from_rgb(100, 100, 110),
        );
    }

    // Render axis orientation gizmo in 3D mode
    if viewport.viewport_mode == ViewportMode::Mode3D {
        render_axis_gizmo(&ctx, orbit, content_rect);
    }

    // Handle asset drag and drop from assets panel
    let pointer_pos = ctx.pointer_hover_pos();
    let in_viewport = pointer_pos.map_or(false, |p| content_rect.contains(p));

    if assets.dragging_asset.is_some() && ctx.input(|i| i.pointer.any_released()) {
        if in_viewport {
            if let Some(asset_path) = assets.dragging_asset.take() {
                if let Some(mouse_pos) = pointer_pos {
                    let local_x = mouse_pos.x - content_rect.min.x;
                    let local_y = mouse_pos.y - content_rect.min.y;

                    let norm_x = local_x / content_rect.width();
                    let norm_y = local_y / content_rect.height();

                    // Check if this is an image file
                    let is_image = is_image_file(&asset_path);
                    let is_2d_mode = viewport.viewport_mode == ViewportMode::Mode2D;

                    if is_2d_mode {
                        // In 2D mode, calculate position based on 2D camera
                        // For simplicity, use viewport center as origin with offset based on mouse position
                        let pos_x = (norm_x - 0.5) * content_rect.width();
                        let pos_y = (0.5 - norm_y) * content_rect.height(); // Invert Y for screen coords

                        if is_image {
                            assets.pending_image_drop = Some(PendingImageDrop {
                                path: asset_path,
                                position: Vec3::new(pos_x, pos_y, 0.0),
                                is_2d_mode: true,
                            });
                        }
                        // Note: Models in 2D mode not supported yet
                    } else {
                        // 3D mode - calculate ground plane intersection
                        let camera_pos = calculate_camera_position(
                            orbit.focus,
                            orbit.distance,
                            orbit.yaw,
                            orbit.pitch,
                        );

                        let fov = std::f32::consts::FRAC_PI_4;
                        let aspect = content_rect.width() / content_rect.height();

                        let ndc_x = norm_x * 2.0 - 1.0;
                        let ndc_y = 1.0 - norm_y * 2.0;

                        let tan_fov = (fov / 2.0).tan();
                        let ray_view = Vec3::new(
                            ndc_x * tan_fov * aspect,
                            ndc_y * tan_fov,
                            -1.0,
                        )
                        .normalize();

                        let camera_forward = (orbit.focus - camera_pos).normalize();
                        let camera_right = camera_forward.cross(Vec3::Y).normalize();
                        let camera_up = camera_right.cross(camera_forward).normalize();

                        let ray_world = (camera_right * ray_view.x
                            + camera_up * ray_view.y
                            - camera_forward * ray_view.z)
                            .normalize();

                        let ground_point = if ray_world.y.abs() > 0.0001 {
                            let t = -camera_pos.y / ray_world.y;
                            if t > 0.0 {
                                camera_pos + ray_world * t
                            } else {
                                Vec3::ZERO
                            }
                        } else {
                            Vec3::ZERO
                        };

                        if is_image {
                            assets.pending_image_drop = Some(PendingImageDrop {
                                path: asset_path,
                                position: ground_point,
                                is_2d_mode: false,
                            });
                        } else {
                            assets.pending_asset_drop = Some((asset_path, ground_point));
                        }
                    }
                }
            }
        } else {
            // Released outside viewport, cancel the drag
            assets.dragging_asset = None;
        }
    }
}

/// Check if a file is an image based on extension
fn is_image_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            let ext_lower = ext.to_lowercase();
            matches!(ext_lower.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp")
        })
        .unwrap_or(false)
}

/// Calculate camera position from orbit parameters
fn calculate_camera_position(focus: Vec3, distance: f32, yaw: f32, pitch: f32) -> Vec3 {
    let x = focus.x + distance * pitch.cos() * yaw.sin();
    let y = focus.y + distance * pitch.sin();
    let z = focus.z + distance * pitch.cos() * yaw.cos();
    Vec3::new(x, y, z)
}

/// Render the axis orientation gizmo in the top right corner of the 3D viewport (Blender-style)
fn render_axis_gizmo(ctx: &egui::Context, orbit: &mut OrbitCameraState, content_rect: Rect) {
    // Center of the gizmo (top right corner with margin)
    let center = Pos2::new(
        content_rect.max.x - AXIS_GIZMO_SIZE / 2.0 - AXIS_GIZMO_MARGIN,
        content_rect.min.y + AXIS_GIZMO_SIZE / 2.0 + AXIS_GIZMO_MARGIN,
    );

    // Calculate camera view matrix components
    let cos_yaw = orbit.yaw.cos();
    let sin_yaw = orbit.yaw.sin();
    let cos_pitch = orbit.pitch.cos();
    let sin_pitch = orbit.pitch.sin();

    // Define world axes with their colors and view directions (yaw, pitch)
    // Positive axes
    let axes = [
        (Vec3::X, Color32::from_rgb(237, 76, 92), "X", std::f32::consts::FRAC_PI_2, 0.0),      // +X: look from +X toward origin
        (Vec3::Y, Color32::from_rgb(139, 201, 63), "Y", 0.0, std::f32::consts::FRAC_PI_2),     // +Y: look from top
        (Vec3::Z, Color32::from_rgb(68, 138, 255), "Z", 0.0, 0.0),                              // +Z: look from +Z toward origin
        (-Vec3::X, Color32::from_rgb(150, 50, 60), "-X", -std::f32::consts::FRAC_PI_2, 0.0),   // -X
        (-Vec3::Y, Color32::from_rgb(80, 120, 40), "-Y", 0.0, -std::f32::consts::FRAC_PI_2),   // -Y: look from bottom
        (-Vec3::Z, Color32::from_rgb(40, 80, 150), "-Z", std::f32::consts::PI, 0.0),           // -Z
    ];

    let axis_length = AXIS_GIZMO_SIZE / 2.0 - 12.0;
    let click_radius = 12.0;

    // Project each axis to screen space
    let mut projected_axes: Vec<(f32, Vec2, Color32, &str, f32, f32, bool)> = axes
        .iter()
        .map(|(world_axis, color, label, target_yaw, target_pitch)| {
            // Transform world axis to view space
            let rotated_yaw = Vec3::new(
                world_axis.x * cos_yaw + world_axis.z * sin_yaw,
                world_axis.y,
                -world_axis.x * sin_yaw + world_axis.z * cos_yaw,
            );

            let view_axis = Vec3::new(
                rotated_yaw.x,
                rotated_yaw.y * cos_pitch + rotated_yaw.z * sin_pitch,
                -rotated_yaw.y * sin_pitch + rotated_yaw.z * cos_pitch,
            );

            let screen_offset = Vec2::new(view_axis.x * axis_length, -view_axis.y * axis_length);
            let is_positive = world_axis.x > 0.0 || world_axis.y > 0.0 || world_axis.z > 0.0;

            (view_axis.z, screen_offset, *color, *label, *target_yaw, *target_pitch, is_positive)
        })
        .collect();

    // Sort by depth (back to front)
    projected_axes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Check for mouse interaction
    let mouse_pos = ctx.pointer_hover_pos();
    let clicked = ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));

    // Use an area for the gizmo overlay
    egui::Area::new(egui::Id::new("axis_gizmo"))
        .fixed_pos(Pos2::new(center.x - AXIS_GIZMO_SIZE / 2.0, center.y - AXIS_GIZMO_SIZE / 2.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let painter = ui.painter();

            // Draw axes
            for (depth, offset, color, label, target_yaw, target_pitch, is_positive) in &projected_axes {
                let end_pos = Pos2::new(center.x + offset.x, center.y + offset.y);

                // Check if mouse is hovering over this axis end
                let is_hovered = mouse_pos.map_or(false, |mp| {
                    let dist = ((mp.x - end_pos.x).powi(2) + (mp.y - end_pos.y).powi(2)).sqrt();
                    dist < click_radius
                });

                // Handle click
                if is_hovered && clicked {
                    orbit.yaw = *target_yaw;
                    orbit.pitch = *target_pitch;
                }

                // Fade axes that point away from camera
                let base_alpha = if *depth < -0.1 { 100 } else { 255 };
                let alpha = if is_hovered { 255 } else { base_alpha };
                let axis_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);

                // Draw axis line - thicker for positive axes
                let line_width = if *is_positive {
                    if *depth < -0.1 { 2.0 } else { 3.0 }
                } else {
                    if *depth < -0.1 { 1.0 } else { 1.5 }
                };

                // Only draw lines for positive axes (negative ones just have end caps)
                if *is_positive {
                    painter.line_segment([center, end_pos], Stroke::new(line_width, axis_color));
                }

                // Draw axis end cap - filled circle for positive, ring for negative
                let cap_size = if is_hovered {
                    if *is_positive { 11.0 } else { 8.0 }
                } else {
                    if *is_positive { 9.0 } else { 6.0 }
                };

                if *is_positive {
                    painter.circle_filled(end_pos, cap_size, axis_color);
                    // Draw label on the cap
                    painter.text(
                        end_pos,
                        egui::Align2::CENTER_CENTER,
                        *label,
                        FontId::proportional(11.0),
                        Color32::WHITE,
                    );
                } else {
                    // Negative axis - smaller hollow circle
                    painter.circle_stroke(end_pos, cap_size, Stroke::new(2.0, axis_color));
                }
            }

            // Draw center dot
            painter.circle_filled(center, 3.0, Color32::from_rgb(180, 180, 180));
        });
}

/// Render the box selection rectangle
fn render_box_selection(ctx: &egui::Context, gizmo: &GizmoState) {
    let (min_x, min_y, max_x, max_y) = gizmo.box_selection.get_rect();

    let rect = Rect::from_min_max(
        Pos2::new(min_x, min_y),
        Pos2::new(max_x, max_y),
    );

    egui::Area::new(egui::Id::new("box_selection"))
        .fixed_pos(rect.min)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter();

            // Fill with semi-transparent blue
            painter.rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(66, 150, 250, 40),
            );

            // Draw border
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, Color32::from_rgb(66, 150, 250)),
                egui::StrokeKind::Outside,
            );
        });
}

/// Render the modal transform HUD overlay
fn render_modal_transform_hud(ctx: &egui::Context, modal: &ModalTransformState, viewport_rect: Rect) {
    let Some(mode) = &modal.mode else { return };

    // Position HUD at bottom of viewport
    let hud_height = 60.0;
    let hud_width = 300.0;
    let hud_rect = Rect::from_min_size(
        Pos2::new(
            viewport_rect.center().x - hud_width / 2.0,
            viewport_rect.max.y - hud_height - 10.0,
        ),
        Vec2::new(hud_width, hud_height),
    );

    egui::Area::new(egui::Id::new("modal_transform_hud"))
        .fixed_pos(hud_rect.min)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter();

            // Background
            painter.rect_filled(
                hud_rect,
                8.0,
                Color32::from_rgba_unmultiplied(30, 30, 35, 230),
            );
            painter.rect_stroke(
                hud_rect,
                8.0,
                Stroke::new(1.0, Color32::from_rgb(60, 60, 70)),
                egui::StrokeKind::Outside,
            );

            // Mode text (large, centered at top)
            let mode_text = mode.display_name();
            painter.text(
                Pos2::new(hud_rect.center().x, hud_rect.min.y + 16.0),
                egui::Align2::CENTER_CENTER,
                mode_text,
                FontId::proportional(16.0),
                Color32::WHITE,
            );

            // Axis constraint (with color)
            let axis_text = modal.axis_constraint.display_name();
            if !axis_text.is_empty() {
                let axis_color = axis_constraint_to_color32(modal.axis_constraint);
                painter.text(
                    Pos2::new(hud_rect.center().x, hud_rect.min.y + 32.0),
                    egui::Align2::CENTER_CENTER,
                    format!("Axis: {}", axis_text),
                    FontId::proportional(12.0),
                    axis_color,
                );
            }

            // Numeric input display
            let numeric_display = modal.numeric_input.display();
            if !numeric_display.is_empty() {
                painter.text(
                    Pos2::new(hud_rect.center().x, hud_rect.min.y + 44.0),
                    egui::Align2::CENTER_CENTER,
                    format!("Value: {}", numeric_display),
                    FontId::proportional(12.0),
                    Color32::from_rgb(100, 200, 255),
                );
            }

            // Help text at bottom
            painter.text(
                Pos2::new(hud_rect.center().x, hud_rect.max.y - 8.0),
                egui::Align2::CENTER_CENTER,
                "X/Y/Z axis | Enter confirm | Esc cancel",
                FontId::proportional(10.0),
                Color32::from_rgb(120, 120, 130),
            );
        });
}

/// Convert AxisConstraint to egui Color32
fn axis_constraint_to_color32(axis: AxisConstraint) -> Color32 {
    match axis {
        AxisConstraint::None => Color32::WHITE,
        AxisConstraint::X | AxisConstraint::PlaneYZ => Color32::from_rgb(237, 76, 92),   // Red
        AxisConstraint::Y | AxisConstraint::PlaneXZ => Color32::from_rgb(139, 201, 63),  // Green
        AxisConstraint::Z | AxisConstraint::PlaneXY => Color32::from_rgb(68, 138, 255),  // Blue
    }
}
