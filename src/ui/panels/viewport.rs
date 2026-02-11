use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, TextureId, Vec2};

use crate::core::{ViewportMode, ViewportState, AssetBrowserState, OrbitCameraState, GizmoState, PendingImageDrop, PendingMaterialDrop, EditorSettings, VisualizationMode, ProjectionMode, CameraSettings, SelectionState, HierarchyState};
use crate::commands::{CommandHistory, DeleteEntityCommand, DuplicateEntityCommand, queue_command};
use crate::gizmo::{EditorTool, GizmoMode, ModalTransformState, AxisConstraint, SnapSettings};
use crate::terrain::TerrainSculptState;
use crate::viewport::Camera2DState;
use crate::theming::Theme;

use egui_phosphor::regular::{
    ARROWS_OUT_CARDINAL, ARROW_CLOCKWISE, ARROWS_OUT, CURSOR, MAGNET, CARET_DOWN,
    IMAGE, POLYGON, SUN, CLOUD, EYE, CUBE, VIDEO_CAMERA,
    COPY, CLIPBOARD, PLUS, TRASH, PALETTE, FILM_SCRIPT, FOLDER_PLUS, SCROLL, DOWNLOAD,
    CARET_RIGHT, BLUEPRINT, GRAPHICS_CARD, SPARKLE,
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
    terrain_sculpt_state: &TerrainSculptState,
    left_panel_width: f32,
    right_panel_width: f32,
    content_start_y: f32,
    _window_size: [f32; 2],
    content_height: f32,
    viewport_texture_id: Option<TextureId>,
    theme: &Theme,
    selection: &SelectionState,
    hierarchy: &mut HierarchyState,
    command_history: &mut CommandHistory,
    in_play_mode: bool,
) {
    let screen_rect = ctx.content_rect();

    // Calculate the full viewport area
    let full_viewport_rect = Rect::from_min_size(
        Pos2::new(left_panel_width, content_start_y),
        Vec2::new(
            screen_rect.width() - left_panel_width - right_panel_width,
            content_height,
        ),
    );

    // In play mode, skip tabs/rulers and use full area as content
    let content_rect = if in_play_mode {
        full_viewport_rect
    } else {
        // Render viewport mode tabs at the top (with tools)
        render_viewport_tabs(ctx, viewport, orbit, gizmo, settings, full_viewport_rect, theme);

        // Calculate content rect (below tabs, accounting for rulers in 2D mode)
        let tabs_offset = VIEWPORT_TABS_HEIGHT;
        let ruler_offset = if viewport.viewport_mode == ViewportMode::Mode2D { RULER_SIZE } else { 0.0 };

        // Render rulers in 2D mode
        if viewport.viewport_mode == ViewportMode::Mode2D {
            render_rulers(ctx, viewport, camera2d_state, full_viewport_rect, tabs_offset, theme);
        }

        Rect::from_min_size(
            Pos2::new(full_viewport_rect.min.x + ruler_offset, full_viewport_rect.min.y + tabs_offset + ruler_offset),
            Vec2::new(
                full_viewport_rect.width() - ruler_offset,
                full_viewport_rect.height() - tabs_offset - ruler_offset,
            ),
        )
    };

    // Use an Area to render the viewport content
    // Use Order::Middle so it can receive input (positioned below tab bar so doesn't overlap)
    egui::Area::new(egui::Id::new("viewport_area"))
        .fixed_pos(content_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(content_rect);
            render_viewport_content(ui, viewport, assets, orbit, gizmo, terrain_sculpt_state, viewport_texture_id, content_rect, in_play_mode);
        });

    // Skip editor overlays in play mode
    if !in_play_mode {
        // Render box selection rectangle (if active)
        if gizmo.box_selection.active && gizmo.box_selection.is_drag() {
            render_box_selection(ctx, gizmo);
        }

        // Render modal transform HUD (if active)
        if modal_transform.active {
            render_modal_transform_hud(ctx, modal_transform, content_rect);
        }

        // Render viewport right-click context menu
        render_viewport_context_menu(ctx, viewport, assets, selection, hierarchy, command_history, theme);
    }
}

/// Render the 3D/2D mode tabs at the top of the viewport with tool controls
fn render_viewport_tabs(
    ctx: &egui::Context,
    viewport: &mut ViewportState,
    orbit: &mut OrbitCameraState,
    gizmo: &mut GizmoState,
    settings: &mut EditorSettings,
    full_rect: Rect,
    theme: &Theme,
) {
    let tabs_rect = Rect::from_min_size(
        full_rect.min,
        Vec2::new(full_rect.width(), VIEWPORT_TABS_HEIGHT),
    );

    let active_color = theme.semantic.accent.to_color32();
    let inactive_color = theme.widgets.inactive_bg.to_color32();
    let hovered_color = theme.widgets.hovered_bg.to_color32();
    let _border_color = theme.widgets.border.to_color32();
    let text_color = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let button_size = Vec2::new(28.0, 20.0);
    let button_y = tabs_rect.min.y + (VIEWPORT_TABS_HEIGHT - button_size.y) / 2.0;
    let tab_height = VIEWPORT_TABS_HEIGHT - 4.0;
    let tab_y = tabs_rect.min.y + 2.0;
    let tab_width = 36.0;

    // Calculate section widths
    // Left section: 3D/2D tabs
    let _left_section_width = tab_width * 2.0 + 2.0 + 8.0; // Two tabs + gap + padding

    // Center section: Select + Transform tools (4 buttons) + Snap dropdown
    let center_section_width = button_size.x * 4.0 + 2.0 * 3.0 + 4.0 + 36.0; // 4 buttons + gaps + snap dropdown

    // Right section: 4 render toggles + viz dropdown + view angles dropdown + camera settings dropdown
    let right_section_width = button_size.x * 4.0 + 2.0 * 3.0 + 4.0 + 36.0 + 4.0 + 36.0 + 4.0 + 36.0; // 4 buttons + gaps + viz dropdown + view dropdown + camera dropdown

    // Calculate positions
    let left_start = tabs_rect.min.x + 4.0;
    let center_start = tabs_rect.center().x - center_section_width / 2.0;
    let right_start = tabs_rect.max.x - right_section_width - 4.0;

    egui::Area::new(egui::Id::new("viewport_tabs"))
        .fixed_pos(tabs_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(tabs_rect);

            // Background
            ui.painter().rect_filled(tabs_rect, 0.0, theme.surfaces.panel.to_color32());

            // === LEFT SECTION: 3D/2D Mode Tabs ===
            let mut x_pos = left_start;

            // 3D tab
            let tab_3d_rect = Rect::from_min_size(
                Pos2::new(x_pos, tab_y),
                Vec2::new(tab_width, tab_height),
            );
            let is_3d_active = viewport.viewport_mode == ViewportMode::Mode3D;
            let tab_3d_color = if is_3d_active {
                theme.widgets.active_bg.to_color32()
            } else {
                inactive_color
            };
            ui.painter().rect_filled(tab_3d_rect, 3.0, tab_3d_color);
            ui.painter().text(
                tab_3d_rect.center(),
                egui::Align2::CENTER_CENTER,
                "3D",
                FontId::proportional(11.0),
                if is_3d_active { text_color } else { text_muted },
            );
            x_pos += tab_width + 2.0;

            // 2D tab
            let tab_2d_rect = Rect::from_min_size(
                Pos2::new(x_pos, tab_y),
                Vec2::new(tab_width, tab_height),
            );
            let is_2d_active = viewport.viewport_mode == ViewportMode::Mode2D;
            let tab_2d_color = if is_2d_active {
                theme.widgets.active_bg.to_color32()
            } else {
                inactive_color
            };
            ui.painter().rect_filled(tab_2d_rect, 3.0, tab_2d_color);
            ui.painter().text(
                tab_2d_rect.center(),
                egui::Align2::CENTER_CENTER,
                "2D",
                FontId::proportional(11.0),
                if is_2d_active { text_color } else { text_muted },
            );

            // === CENTER SECTION: Transform Tools ===
            x_pos = center_start;

            // Selection Tool
            let is_select = gizmo.tool == EditorTool::Select;
            let select_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let select_resp = viewport_tool_button(ui, select_rect, CURSOR, is_select, active_color, inactive_color, hovered_color);
            if select_resp.clicked() {
                gizmo.tool = EditorTool::Select;
            }
            select_resp.on_hover_text("Select (Q)");
            x_pos += button_size.x + 2.0;

            // Translate
            let is_translate = gizmo.tool == EditorTool::Transform && gizmo.mode == GizmoMode::Translate;
            let translate_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let translate_resp = viewport_tool_button(ui, translate_rect, ARROWS_OUT_CARDINAL, is_translate, active_color, inactive_color, hovered_color);
            if translate_resp.clicked() {
                gizmo.tool = EditorTool::Transform;
                gizmo.mode = GizmoMode::Translate;
            }
            translate_resp.on_hover_text("Move (W)");
            x_pos += button_size.x + 2.0;

            // Rotate
            let is_rotate = gizmo.tool == EditorTool::Transform && gizmo.mode == GizmoMode::Rotate;
            let rotate_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let rotate_resp = viewport_tool_button(ui, rotate_rect, ARROW_CLOCKWISE, is_rotate, active_color, inactive_color, hovered_color);
            if rotate_resp.clicked() {
                gizmo.tool = EditorTool::Transform;
                gizmo.mode = GizmoMode::Rotate;
            }
            rotate_resp.on_hover_text("Rotate (E)");
            x_pos += button_size.x + 2.0;

            // Scale
            let is_scale = gizmo.tool == EditorTool::Transform && gizmo.mode == GizmoMode::Scale;
            let scale_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let scale_resp = viewport_tool_button(ui, scale_rect, ARROWS_OUT, is_scale, active_color, inactive_color, hovered_color);
            if scale_resp.clicked() {
                gizmo.tool = EditorTool::Transform;
                gizmo.mode = GizmoMode::Scale;
            }
            scale_resp.on_hover_text("Scale (R)");
            x_pos += button_size.x + 4.0;

            // Snap Dropdown
            let any_snap_enabled = gizmo.snap.translate_enabled || gizmo.snap.rotate_enabled || gizmo.snap.scale_enabled || gizmo.snap.object_snap_enabled || gizmo.snap.floor_snap_enabled;
            let snap_color = if any_snap_enabled {
                active_color
            } else {
                text_muted
            };
            let snap_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), Vec2::new(36.0, button_size.y));
            viewport_snap_dropdown(ui, ctx, snap_rect, MAGNET, snap_color, inactive_color, hovered_color, text_muted, &mut gizmo.snap, theme);

            // === RIGHT SECTION: Render Settings ===
            x_pos = right_start;

            let toggle_on_color = active_color;
            let toggle_off_color = inactive_color;

            // Textures toggle
            let tex_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let tex_resp = viewport_tool_button(ui, tex_rect, IMAGE, settings.render_toggles.textures, toggle_on_color, toggle_off_color, hovered_color);
            if tex_resp.clicked() {
                settings.render_toggles.textures = !settings.render_toggles.textures;
            }
            tex_resp.on_hover_text(if settings.render_toggles.textures { "Textures: ON" } else { "Textures: OFF" });
            x_pos += button_size.x + 2.0;

            // Wireframe toggle
            let wire_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let wire_resp = viewport_tool_button(ui, wire_rect, POLYGON, settings.render_toggles.wireframe, toggle_on_color, toggle_off_color, hovered_color);
            if wire_resp.clicked() {
                settings.render_toggles.wireframe = !settings.render_toggles.wireframe;
            }
            wire_resp.on_hover_text(if settings.render_toggles.wireframe { "Wireframe: ON" } else { "Wireframe: OFF" });
            x_pos += button_size.x + 2.0;

            // Lighting toggle
            let light_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let light_resp = viewport_tool_button(ui, light_rect, SUN, settings.render_toggles.lighting, toggle_on_color, toggle_off_color, hovered_color);
            if light_resp.clicked() {
                settings.render_toggles.lighting = !settings.render_toggles.lighting;
            }
            light_resp.on_hover_text(if settings.render_toggles.lighting { "Lighting: ON" } else { "Lighting: OFF" });
            x_pos += button_size.x + 2.0;

            // Shadows toggle
            let shadow_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), button_size);
            let shadow_resp = viewport_tool_button(ui, shadow_rect, CLOUD, settings.render_toggles.shadows, toggle_on_color, toggle_off_color, hovered_color);
            if shadow_resp.clicked() {
                settings.render_toggles.shadows = !settings.render_toggles.shadows;
            }
            shadow_resp.on_hover_text(if settings.render_toggles.shadows { "Shadows: ON" } else { "Shadows: OFF" });
            x_pos += button_size.x + 4.0;

            // Visualization Mode Dropdown
            let viz_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), Vec2::new(36.0, button_size.y));
            let viz_color = theme.text.secondary.to_color32();
            viewport_viz_dropdown(ui, ctx, viz_rect, EYE, viz_color, inactive_color, hovered_color, text_muted, settings);
            x_pos += 36.0 + 4.0;

            // View Angles Dropdown
            let view_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), Vec2::new(36.0, button_size.y));
            let view_color = theme.text.secondary.to_color32();
            viewport_view_dropdown(ui, ctx, view_rect, CUBE, view_color, inactive_color, hovered_color, text_muted, orbit);
            x_pos += 36.0 + 4.0;

            // Camera Settings Dropdown
            let camera_rect = Rect::from_min_size(Pos2::new(x_pos, button_y), Vec2::new(36.0, button_size.y));
            let camera_color = theme.text.secondary.to_color32();
            viewport_camera_dropdown(ui, ctx, camera_rect, VIDEO_CAMERA, camera_color, inactive_color, hovered_color, text_muted, settings, theme);

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
    hovered_color: Color32,
) -> egui::Response {
    let response = ui.allocate_rect(rect, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let bg_color = if active {
            active_color
        } else if response.hovered() {
            hovered_color
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
    hovered_color: Color32,
    caret_color: Color32,
    snap: &mut SnapSettings,
    theme: &Theme,
) {
    let button_id = ui.make_persistent_id("viewport_snap_dropdown");
    let response = ui.allocate_rect(rect, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            hovered_color
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
            caret_color,
        );
    }

    if response.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(button_id));
    }

    let active_btn_color = theme.semantic.accent.to_color32();
    let inactive_btn_color = theme.widgets.inactive_bg.to_color32();
    let label_color = theme.text.muted.to_color32();

    #[allow(deprecated)]
    egui::popup_below_widget(
        ui,
        button_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(180.0);
            ui.style_mut().spacing.item_spacing.y = 4.0;

            ui.label(RichText::new("Snapping").small().color(label_color));
            ui.add_space(4.0);

            // Position snap
            ui.horizontal(|ui| {
                let pos_active = snap.translate_enabled;
                if ui.add(
                    egui::Button::new(RichText::new("Position").size(12.0))
                        .fill(if pos_active { active_btn_color } else { inactive_btn_color })
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
                        .fill(if rot_active { active_btn_color } else { inactive_btn_color })
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
                        .fill(if scale_active { active_btn_color } else { inactive_btn_color })
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

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(RichText::new("Object Snapping").small().color(label_color));
            ui.add_space(4.0);

            // Object snap (snap to nearby entities)
            ui.horizontal(|ui| {
                let obj_snap_active = snap.object_snap_enabled;
                if ui.add(
                    egui::Button::new(RichText::new("Objects").size(12.0))
                        .fill(if obj_snap_active { active_btn_color } else { inactive_btn_color })
                        .min_size(Vec2::new(70.0, 20.0))
                ).clicked() {
                    snap.object_snap_enabled = !snap.object_snap_enabled;
                }

                ui.add(
                    egui::DragValue::new(&mut snap.object_snap_distance)
                        .range(0.1..=10.0)
                        .speed(0.1)
                        .max_decimals(1)
                        .suffix(" dist")
                );
            });

            // Floor snap
            ui.horizontal(|ui| {
                let floor_snap_active = snap.floor_snap_enabled;
                if ui.add(
                    egui::Button::new(RichText::new("Floor").size(12.0))
                        .fill(if floor_snap_active { active_btn_color } else { inactive_btn_color })
                        .min_size(Vec2::new(70.0, 20.0))
                ).clicked() {
                    snap.floor_snap_enabled = !snap.floor_snap_enabled;
                }

                ui.add(
                    egui::DragValue::new(&mut snap.floor_y)
                        .range(-1000.0..=1000.0)
                        .speed(0.1)
                        .max_decimals(1)
                        .suffix(" Y")
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
    hovered_color: Color32,
    caret_color: Color32,
    settings: &mut EditorSettings,
) {
    let button_id = ui.make_persistent_id("viewport_viz_dropdown");
    let response = ui.allocate_rect(rect, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            hovered_color
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
            caret_color,
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

/// View angle preset for the camera
#[derive(Clone, Copy, PartialEq)]
pub enum ViewAngle {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

impl ViewAngle {
    pub const ALL: &'static [ViewAngle] = &[
        ViewAngle::Front,
        ViewAngle::Back,
        ViewAngle::Left,
        ViewAngle::Right,
        ViewAngle::Top,
        ViewAngle::Bottom,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ViewAngle::Front => "Front",
            ViewAngle::Back => "Back",
            ViewAngle::Left => "Left",
            ViewAngle::Right => "Right",
            ViewAngle::Top => "Top",
            ViewAngle::Bottom => "Bottom",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            ViewAngle::Front => "Num1",
            ViewAngle::Back => "Ctrl+Num1",
            ViewAngle::Left => "Ctrl+Num3",
            ViewAngle::Right => "Num3",
            ViewAngle::Top => "Num7",
            ViewAngle::Bottom => "Ctrl+Num7",
        }
    }

    /// Get yaw and pitch for this view angle
    pub fn yaw_pitch(&self) -> (f32, f32) {
        match self {
            ViewAngle::Front => (0.0, 0.0),                                          // Looking from +Z toward origin
            ViewAngle::Back => (std::f32::consts::PI, 0.0),                           // Looking from -Z toward origin
            ViewAngle::Right => (std::f32::consts::FRAC_PI_2, 0.0),                   // Looking from +X toward origin
            ViewAngle::Left => (-std::f32::consts::FRAC_PI_2, 0.0),                   // Looking from -X toward origin
            ViewAngle::Top => (0.0, std::f32::consts::FRAC_PI_2),                     // Looking from top
            ViewAngle::Bottom => (0.0, -std::f32::consts::FRAC_PI_2),                 // Looking from bottom
        }
    }
}

/// View angles dropdown for viewport header
fn viewport_view_dropdown(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    rect: Rect,
    icon: &str,
    icon_color: Color32,
    bg_color: Color32,
    hovered_color: Color32,
    caret_color: Color32,
    orbit: &mut OrbitCameraState,
) {
    let button_id = ui.make_persistent_id("viewport_view_dropdown");
    let response = ui.allocate_rect(rect, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            hovered_color
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
            caret_color,
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
            ui.set_min_width(160.0);
            ui.style_mut().spacing.item_spacing.y = 2.0;

            // Projection toggle section
            ui.label(RichText::new("Projection").small().color(egui::Color32::from_gray(140)));
            ui.add_space(2.0);

            let persp_selected = orbit.projection_mode == ProjectionMode::Perspective;
            let persp_label = if persp_selected { "• Perspective" } else { "  Perspective" };
            if ui.add(
                egui::Button::new(persp_label)
                    .fill(Color32::TRANSPARENT)
                    .corner_radius(CornerRadius::same(2))
                    .min_size(Vec2::new(ui.available_width(), 0.0))
            ).clicked() {
                orbit.projection_mode = ProjectionMode::Perspective;
            }

            let ortho_selected = orbit.projection_mode == ProjectionMode::Orthographic;
            let ortho_label = if ortho_selected { "• Orthographic" } else { "  Orthographic" };
            if ui.add(
                egui::Button::new(ortho_label)
                    .fill(Color32::TRANSPARENT)
                    .corner_radius(CornerRadius::same(2))
                    .min_size(Vec2::new(ui.available_width(), 0.0))
            ).clicked() {
                orbit.projection_mode = ProjectionMode::Orthographic;
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // View angles section
            ui.label(RichText::new("View Angles").small().color(egui::Color32::from_gray(140)));
            ui.add_space(2.0);

            for view in ViewAngle::ALL {
                let label = format!("{}  ({})", view.label(), view.shortcut());
                if ui.add(
                    egui::Button::new(&label)
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(CornerRadius::same(2))
                        .min_size(Vec2::new(ui.available_width(), 0.0))
                ).clicked() {
                    let (yaw, pitch) = view.yaw_pitch();
                    orbit.yaw = yaw;
                    orbit.pitch = pitch;
                    ui.close();
                }
            }
        },
    );

    response.on_hover_text("View Angle");
}

/// Camera settings dropdown for viewport header
fn viewport_camera_dropdown(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    rect: Rect,
    icon: &str,
    icon_color: Color32,
    bg_color: Color32,
    hovered_color: Color32,
    caret_color: Color32,
    settings: &mut EditorSettings,
    theme: &Theme,
) {
    let button_id = ui.make_persistent_id("viewport_camera_dropdown");
    let response = ui.allocate_rect(rect, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            hovered_color
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
            caret_color,
        );
    }

    if response.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(button_id));
    }

    let label_color = theme.text.muted.to_color32();

    #[allow(deprecated)]
    egui::popup_below_widget(
        ui,
        button_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(200.0);
            ui.style_mut().spacing.item_spacing.y = 4.0;

            ui.label(RichText::new("Camera Settings").small().color(label_color));
            ui.add_space(4.0);

            // Move Speed
            ui.horizontal(|ui| {
                ui.label(RichText::new("Move Speed").size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut settings.camera_settings.move_speed)
                            .range(0.1..=100.0)
                            .speed(0.5)
                            .max_decimals(1)
                    );
                });
            });

            // Look Sensitivity
            ui.horizontal(|ui| {
                ui.label(RichText::new("Look Sensitivity").size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut settings.camera_settings.look_sensitivity)
                            .range(0.05..=2.0)
                            .speed(0.05)
                            .max_decimals(2)
                    );
                });
            });

            // Orbit Sensitivity
            ui.horizontal(|ui| {
                ui.label(RichText::new("Orbit Sensitivity").size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut settings.camera_settings.orbit_sensitivity)
                            .range(0.05..=2.0)
                            .speed(0.05)
                            .max_decimals(2)
                    );
                });
            });

            // Pan Sensitivity
            ui.horizontal(|ui| {
                ui.label(RichText::new("Pan Sensitivity").size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut settings.camera_settings.pan_sensitivity)
                            .range(0.1..=5.0)
                            .speed(0.1)
                            .max_decimals(1)
                    );
                });
            });

            // Zoom Sensitivity
            ui.horizontal(|ui| {
                ui.label(RichText::new("Zoom Sensitivity").size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut settings.camera_settings.zoom_sensitivity)
                            .range(0.1..=5.0)
                            .speed(0.1)
                            .max_decimals(1)
                    );
                });
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Invert Y Axis
            ui.horizontal(|ui| {
                ui.label(RichText::new("Invert Y Axis").size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut settings.camera_settings.invert_y, "");
                });
            });

            // Distance Relative Speed
            ui.horizontal(|ui| {
                ui.label(RichText::new("Distance Relative Speed").size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut settings.camera_settings.distance_relative_speed, "");
                });
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Reset to Defaults button
            if ui.add(
                egui::Button::new(RichText::new("Reset to Defaults").size(12.0))
                    .min_size(Vec2::new(ui.available_width(), 24.0))
            ).clicked() {
                settings.camera_settings = CameraSettings::default();
            }

            ui.add_space(4.0);

            // Help text
            ui.label(RichText::new("Controls:").small().color(label_color));
            ui.label(RichText::new("• RMB Drag: Look around").small().color(label_color));
            ui.label(RichText::new("• RMB + WASD: Fly mode").small().color(label_color));
            ui.label(RichText::new("• LMB Drag: Move camera").small().color(label_color));
            ui.label(RichText::new("• MMB Drag: Orbit").small().color(label_color));
            ui.label(RichText::new("• Alt + LMB: Orbit").small().color(label_color));
            ui.label(RichText::new("• Scroll: Zoom").small().color(label_color));
        },
    );

    response.on_hover_text("Camera Settings");
}

/// Render rulers for 2D mode
fn render_rulers(
    ctx: &egui::Context,
    viewport: &ViewportState,
    camera2d_state: &Camera2DState,
    full_rect: Rect,
    tabs_offset: f32,
    theme: &Theme,
) {
    let ruler_bg = theme.surfaces.extreme.to_color32();
    let ruler_tick = theme.text.disabled.to_color32();
    let ruler_text = theme.text.muted.to_color32();
    let ruler_major_tick = theme.text.secondary.to_color32();

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
    // viewport.position already accounts for ruler offset
    viewport.position[0] + viewport.size[0] / 2.0 + screen_relative
}

/// Convert world Y coordinate to screen Y coordinate
fn world_to_screen_y(world_y: f32, camera2d_state: &Camera2DState, viewport: &ViewportState) -> f32 {
    let relative_y = world_y - camera2d_state.pan_offset.y;
    let screen_relative = -relative_y * camera2d_state.zoom; // Y is inverted in screen coords
    // viewport.position already accounts for tabs and ruler offset
    viewport.position[1] + viewport.size[1] / 2.0 + screen_relative
}

/// Render viewport content (for use in docking)
pub fn render_viewport_content(
    ui: &mut egui::Ui,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    orbit: &mut OrbitCameraState,
    _gizmo: &GizmoState,
    terrain_sculpt_state: &TerrainSculptState,
    viewport_texture_id: Option<TextureId>,
    content_rect: Rect,
    in_play_mode: bool,
) {
    let ctx = ui.ctx().clone();

    // Update viewport state with the actual content area
    viewport.position = [content_rect.min.x, content_rect.min.y];
    viewport.size = [content_rect.width(), content_rect.height()];
    // Only consider viewport hovered if pointer is inside AND no resize handle is being interacted with
    // Disable viewport hover in play mode to prevent editor camera controls
    viewport.hovered = !in_play_mode && ui.rect_contains_pointer(content_rect) && !viewport.resize_handle_active;

    // Set cursor for viewport - game engine friendly crosshair (skip in play mode)
    if !in_play_mode && viewport.hovered && !terrain_sculpt_state.brush_visible {
        ctx.set_cursor_icon(CursorIcon::Crosshair);
    }

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

    // Render axis orientation gizmo in 3D mode (skip in play mode)
    if !in_play_mode && viewport.viewport_mode == ViewportMode::Mode3D {
        render_axis_gizmo(&ctx, orbit, content_rect);
    }

    // Skip drag-drop in play mode
    if in_play_mode {
        return;
    }

    // Handle asset drag and drop from assets panel
    let pointer_pos = ctx.pointer_hover_pos();
    let in_viewport = pointer_pos.map_or(false, |p| content_rect.contains(p));

    // Update drag_ground_position every frame while dragging a model over the 3D viewport
    if assets.dragging_asset.is_some() && in_viewport && viewport.viewport_mode == ViewportMode::Mode3D {
        if let Some(mouse_pos) = pointer_pos {
            let ground_pos = ground_plane_intersection(orbit, content_rect, mouse_pos);
            assets.drag_ground_position = Some(ground_pos);
        }
    } else {
        assets.drag_ground_position = None;
    }

    if assets.dragging_asset.is_some() && ctx.input(|i| i.pointer.any_released()) {
        if in_viewport {
            if let Some(asset_path) = assets.dragging_asset.take() {
                if let Some(mouse_pos) = pointer_pos {
                    let local_x = mouse_pos.x - content_rect.min.x;
                    let local_y = mouse_pos.y - content_rect.min.y;

                    let norm_x = local_x / content_rect.width();
                    let norm_y = local_y / content_rect.height();

                    // Check file type
                    let is_hdr = is_hdr_file(&asset_path);
                    let is_image = is_image_file(&asset_path);
                    let is_material = is_blueprint_material(&asset_path);
                    let is_effect = asset_path.to_string_lossy().to_lowercase().ends_with(".effect");
                    let is_2d_mode = viewport.viewport_mode == ViewportMode::Mode2D;

                    // Handle HDR/EXR drops → create/update skybox
                    if is_hdr {
                        assets.pending_skybox_drop = Some(asset_path);
                    }
                    // Handle .effect drops → create particle entity at ground point
                    else if is_effect {
                        let ground_point = if is_2d_mode {
                            Vec3::new(
                                (norm_x - 0.5) * content_rect.width(),
                                (0.5 - norm_y) * content_rect.height(),
                                0.0,
                            )
                        } else {
                            ground_plane_intersection(orbit, content_rect, mouse_pos)
                        };
                        assets.pending_effect_drop = Some((asset_path, ground_point));
                    }
                    // Handle material blueprint drops (only in 3D mode)
                    else if is_material && !is_2d_mode {
                        // Store cursor position for entity picking in the system
                        assets.pending_material_drop = Some(PendingMaterialDrop {
                            path: asset_path,
                            cursor_pos: bevy::math::Vec2::new(mouse_pos.x, mouse_pos.y),
                        });
                    } else if is_2d_mode {
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
                        // Note: Models and materials in 2D mode not supported yet
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
        }
        // Note: Don't clear dragging_asset when released outside viewport
        // Other panels (like Inspector) may handle the drop
    }
}

/// Check if a file is an HDR/EXR environment texture
fn is_hdr_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| matches!(ext.to_lowercase().as_str(), "hdr" | "exr"))
        .unwrap_or(false)
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

/// Check if a file is a material blueprint based on extension
fn is_blueprint_material(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.to_lowercase() == "material_bp")
        .unwrap_or(false)
}

/// Compute the ground plane (Y=0) intersection for a mouse position in the 3D viewport.
fn ground_plane_intersection(orbit: &OrbitCameraState, content_rect: Rect, mouse_pos: Pos2) -> Vec3 {
    let local_x = mouse_pos.x - content_rect.min.x;
    let local_y = mouse_pos.y - content_rect.min.y;
    let norm_x = local_x / content_rect.width();
    let norm_y = local_y / content_rect.height();

    let camera_pos = calculate_camera_position(orbit.focus, orbit.distance, orbit.yaw, orbit.pitch);
    let fov = std::f32::consts::FRAC_PI_4;
    let aspect = content_rect.width() / content_rect.height();

    let ndc_x = norm_x * 2.0 - 1.0;
    let ndc_y = 1.0 - norm_y * 2.0;

    let tan_fov = (fov / 2.0).tan();
    let ray_view = Vec3::new(ndc_x * tan_fov * aspect, ndc_y * tan_fov, -1.0).normalize();

    let camera_forward = (orbit.focus - camera_pos).normalize();
    let camera_right = camera_forward.cross(Vec3::Y).normalize();
    let camera_up = camera_right.cross(camera_forward).normalize();

    let ray_world = (camera_right * ray_view.x + camera_up * ray_view.y - camera_forward * ray_view.z).normalize();

    if ray_world.y.abs() > 0.0001 {
        let t = -camera_pos.y / ray_world.y;
        if t > 0.0 { camera_pos + ray_world * t } else { Vec3::ZERO }
    } else {
        Vec3::ZERO
    }
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

/// Render the viewport right-click context menu
fn render_viewport_context_menu(
    ctx: &egui::Context,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    selection: &SelectionState,
    hierarchy: &mut HierarchyState,
    command_history: &mut CommandHistory,
    theme: &Theme,
) {
    let Some(pos) = viewport.context_menu_pos else { return };

    // Close menu on Escape
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
        return;
    }

    // Smart positioning - flip if near screen edge
    let menu_height = 220.0;
    let menu_width = 160.0;
    let screen_rect = ctx.screen_rect();

    let menu_y = if pos.y + menu_height > screen_rect.max.y {
        pos.y - menu_height
    } else {
        pos.y
    };

    let menu_x = if pos.x + menu_width > screen_rect.max.x {
        pos.x - menu_width
    } else {
        pos.x
    };

    let area_response = egui::Area::new(egui::Id::new("viewport_context_menu"))
        .fixed_pos(egui::pos2(menu_x, menu_y))
        .order(egui::Order::Foreground)
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .show(ui, |ui| {
                    render_viewport_context_menu_items(ui, viewport, assets, selection, hierarchy, command_history, theme);
                });
        });

    // Read submenu rect from egui memory
    let submenu_rect: Option<egui::Rect> = ctx.memory(|mem| {
        mem.data.get_temp(egui::Id::new("viewport_submenu_rect"))
    });

    // Close menu when clicking outside of both menu and submenu
    if ctx.input(|i| i.pointer.primary_clicked() || i.pointer.secondary_clicked()) {
        if let Some(click_pos) = ctx.input(|i| i.pointer.interact_pos()) {
            let in_menu = area_response.response.rect.contains(click_pos);
            let in_submenu = submenu_rect.map_or(false, |r| r.contains(click_pos));
            if !in_menu && !in_submenu {
                viewport.context_menu_pos = None;
                viewport.context_submenu = None;
            }
        }
    }
}

/// Render the items inside the viewport context menu
fn render_viewport_context_menu_items(
    ui: &mut egui::Ui,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    selection: &SelectionState,
    hierarchy: &mut HierarchyState,
    command_history: &mut CommandHistory,
    theme: &Theme,
) {
    let menu_width = 160.0;
    ui.set_min_width(menu_width);
    ui.set_max_width(menu_width);

    let text_primary = theme.text.primary.to_color32();
    let text_disabled = theme.text.disabled.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let has_selection = selection.selected_entity.is_some();
    let has_clipboard = viewport.clipboard_entity.is_some();

    let item_height = 20.0;
    let item_font = 11.0;

    // Helper for compact menu items with enabled state
    let menu_item = |ui: &mut egui::Ui, icon: &str, label: &str, color: Color32, enabled: bool| -> bool {
        let desired_size = Vec2::new(menu_width, item_height);
        let (rect, response) = ui.allocate_exact_size(desired_size, if enabled { Sense::click() } else { Sense::hover() });

        let label_color = if enabled { text_primary } else { text_disabled };
        let icon_color = if enabled { color } else { text_disabled };

        if enabled && response.hovered() {
            ui.painter().rect_filled(rect, 3.0, theme.panels.item_hover.to_color32());
        }

        ui.painter().text(
            egui::pos2(rect.min.x + 14.0, rect.center().y),
            egui::Align2::CENTER_CENTER, icon,
            egui::FontId::proportional(item_font), icon_color,
        );
        ui.painter().text(
            egui::pos2(rect.min.x + 28.0, rect.center().y),
            egui::Align2::LEFT_CENTER, label,
            egui::FontId::proportional(item_font), label_color,
        );

        enabled && response.clicked()
    };

    // Section header
    let section_header = |ui: &mut egui::Ui, label: &str| {
        let desired_size = Vec2::new(menu_width, 14.0);
        let (rect, _) = ui.allocate_exact_size(desired_size, Sense::hover());
        ui.painter().text(
            egui::pos2(rect.min.x + 8.0, rect.center().y),
            egui::Align2::LEFT_CENTER, label,
            egui::FontId::proportional(9.0), text_secondary,
        );
    };

    // Submenu trigger
    let submenu_trigger = |ui: &mut egui::Ui, icon: &str, label: &str, color: Color32, submenu_id: &str, current_submenu: &Option<String>| -> (bool, egui::Rect) {
        let desired_size = Vec2::new(menu_width, item_height);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        let is_open = current_submenu.as_deref() == Some(submenu_id);
        if response.hovered() || is_open {
            ui.painter().rect_filled(rect, 3.0, theme.panels.item_hover.to_color32());
        }

        ui.painter().text(
            egui::pos2(rect.min.x + 14.0, rect.center().y),
            egui::Align2::CENTER_CENTER, icon,
            egui::FontId::proportional(item_font), color,
        );
        ui.painter().text(
            egui::pos2(rect.min.x + 28.0, rect.center().y),
            egui::Align2::LEFT_CENTER, label,
            egui::FontId::proportional(item_font), text_primary,
        );
        ui.painter().text(
            egui::pos2(rect.max.x - 10.0, rect.center().y),
            egui::Align2::CENTER_CENTER, CARET_RIGHT,
            egui::FontId::proportional(10.0), text_secondary,
        );

        (response.hovered(), rect)
    };

    ui.add_space(2.0);

    // === Entity Operations ===
    let entity_color = theme.text.secondary.to_color32();

    if menu_item(ui, COPY, "Copy", entity_color, has_selection) {
        viewport.clipboard_entity = selection.selected_entity;
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    if menu_item(ui, CLIPBOARD, "Paste", entity_color, has_clipboard) {
        if let Some(entity) = viewport.clipboard_entity {
            queue_command(command_history, Box::new(DuplicateEntityCommand::new(entity)));
        }
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    if menu_item(ui, COPY, "Duplicate", entity_color, has_selection) {
        if let Some(entity) = selection.selected_entity {
            queue_command(command_history, Box::new(DuplicateEntityCommand::new(entity)));
        }
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    let delete_color = if has_selection { theme.semantic.error.to_color32() } else { text_disabled };
    if menu_item(ui, TRASH, "Delete", delete_color, has_selection) {
        if let Some(entity) = selection.selected_entity {
            queue_command(command_history, Box::new(DeleteEntityCommand::new(entity)));
        }
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    // === Add Node ===
    let add_color = theme.semantic.accent.to_color32();
    if menu_item(ui, PLUS, "Add Node", add_color, true) {
        hierarchy.show_add_entity_popup = true;
        hierarchy.add_entity_parent = None;
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    // === Create section ===
    section_header(ui, "CREATE");
    ui.add_space(1.0);

    let material_color = Color32::from_rgb(0, 200, 83);
    let scene_color = Color32::from_rgb(115, 191, 242);
    let script_color = Color32::from_rgb(255, 128, 0);
    let shader_color = Color32::from_rgb(220, 120, 255);
    let blueprint_color = Color32::from_rgb(100, 160, 255);

    if menu_item(ui, PALETTE, "Material", material_color, true) {
        assets.show_create_material_dialog = true;
        assets.new_material_name = "NewMaterial".to_string();
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    if menu_item(ui, FILM_SCRIPT, "Scene", scene_color, true) {
        assets.show_create_scene_dialog = true;
        assets.new_scene_name = "NewScene".to_string();
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    if menu_item(ui, SCROLL, "Script", script_color, true) {
        assets.show_create_script_dialog = true;
        assets.new_script_name = "new_script".to_string();
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    if menu_item(ui, GRAPHICS_CARD, "Shader", shader_color, true) {
        assets.show_create_shader_dialog = true;
        assets.new_shader_name = "new_shader".to_string();
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    let particle_color = Color32::from_rgb(255, 180, 50);
    if menu_item(ui, SPARKLE, "Particle FX", particle_color, true) {
        assets.show_create_particle_dialog = true;
        assets.new_particle_name = "NewParticle".to_string();
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    // === Blueprint submenu ===
    let (bp_hovered, bp_rect) = submenu_trigger(ui, BLUEPRINT, "Blueprint", blueprint_color, "blueprint", &viewport.context_submenu);

    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    if menu_item(ui, DOWNLOAD, "Import", text_primary, true) {
        assets.show_import_dialog = true;
        viewport.context_menu_pos = None;
        viewport.context_submenu = None;
    }

    ui.add_space(2.0);

    // Track submenu hover
    if bp_hovered {
        viewport.context_submenu = Some("blueprint".to_string());
    }

    // Render blueprint submenu
    let menu_rect = ui.min_rect();
    if viewport.context_submenu.as_deref() == Some("blueprint") {
        let submenu_x = menu_rect.max.x + 1.0;

        let submenu_response = egui::Area::new(egui::Id::new("viewport_submenu_blueprint"))
            .fixed_pos(egui::pos2(submenu_x, bp_rect.min.y))
            .order(egui::Order::Foreground)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        let sub_width = 160.0;
                        ui.set_min_width(sub_width);
                        ui.set_max_width(sub_width);

                        let sub_item = |ui: &mut egui::Ui, icon: &str, label: &str, color: Color32| -> bool {
                            let desired_size = Vec2::new(sub_width, item_height);
                            let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());
                            if response.hovered() {
                                ui.painter().rect_filled(rect, 3.0, theme.panels.item_hover.to_color32());
                            }
                            ui.painter().text(
                                egui::pos2(rect.min.x + 14.0, rect.center().y),
                                egui::Align2::CENTER_CENTER, icon,
                                egui::FontId::proportional(item_font), color,
                            );
                            ui.painter().text(
                                egui::pos2(rect.min.x + 28.0, rect.center().y),
                                egui::Align2::LEFT_CENTER, label,
                                egui::FontId::proportional(item_font), text_primary,
                            );
                            response.clicked()
                        };

                        ui.add_space(2.0);

                        if sub_item(ui, PALETTE, "Material Blueprint", material_color) {
                            assets.show_create_material_blueprint_dialog = true;
                            assets.new_material_blueprint_name = "NewMaterial".to_string();
                            viewport.context_menu_pos = None;
                            viewport.context_submenu = None;
                        }
                        if sub_item(ui, SCROLL, "Script Blueprint", script_color) {
                            assets.show_create_script_blueprint_dialog = true;
                            assets.new_script_blueprint_name = "NewScript".to_string();
                            viewport.context_menu_pos = None;
                            viewport.context_submenu = None;
                        }

                        ui.add_space(2.0);
                    });
            });

        // Store submenu rect for click-outside detection
        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(egui::Id::new("viewport_submenu_rect"), submenu_response.response.rect);
        });

        // Close submenu if pointer moves away from both areas
        if let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
            let expanded_menu = menu_rect.expand(2.0);
            let expanded_submenu = submenu_response.response.rect.expand(2.0);
            if !expanded_menu.contains(pointer_pos) && !expanded_submenu.contains(pointer_pos) {
                viewport.context_submenu = None;
            }
        }
    } else {
        ui.ctx().memory_mut(|mem| {
            mem.data.remove::<egui::Rect>(egui::Id::new("viewport_submenu_rect"));
        });
    }
}
