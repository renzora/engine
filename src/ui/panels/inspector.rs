use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, RichText, TextureId, Vec2};

use crate::component_system::{
    AddComponentPopupState, ComponentCategory, ComponentRegistry,
    get_category_style,
};
use crate::core::{EditorEntity, SelectionState, WorldEnvironmentMarker};
use crate::gizmo::GizmoState;
use crate::ui::inspectors::{
    render_camera_inspector, render_camera_rig_inspector, render_collision_shape_inspector,
    render_directional_light_inspector, render_physics_body_inspector, render_point_light_inspector,
    render_script_inspector, render_spot_light_inspector, render_transform_inspector,
    render_world_environment_inspector,
    // 2D inspectors
    render_sprite2d_inspector, render_camera2d_inspector,
    // UI inspectors
    render_ui_panel_inspector, render_ui_label_inspector, render_ui_button_inspector, render_ui_image_inspector,
};
use crate::shared::{
    CameraNodeData, CameraRigData, CollisionShapeData, PhysicsBodyData,
    // 2D components
    Sprite2DData, Camera2DData,
    // UI components
    UIPanelData, UILabelData, UIButtonData, UIImageData,
};
use crate::plugin_core::{PluginHost, TabLocation};
use crate::scripting::{ScriptComponent, ScriptRegistry, RhaiScriptEngine};
use crate::ui_api::{renderer::UiRenderer, UiEvent};
use super::render_panel_bar;

// Icon for inspector tab
use egui_phosphor::regular::SLIDERS_HORIZONTAL;

// Phosphor icons for inspector
use egui_phosphor::regular::{
    SLIDERS, ARROWS_OUT_CARDINAL, GLOBE, LIGHTBULB, SUN, FLASHLIGHT,
    PLUS, MAGNIFYING_GLASS, CHECK_CIRCLE, CODE, VIDEO_CAMERA, PUZZLE_PIECE,
    CUBE, ATOM, CARET_DOWN, CARET_RIGHT, IMAGE, STACK, TEXTBOX, CURSOR_CLICK, X,
};

/// Background colors for alternating rows
const ROW_BG_EVEN: Color32 = Color32::from_rgb(32, 34, 38);
const ROW_BG_ODD: Color32 = Color32::from_rgb(38, 40, 44);

/// Helper to render a property row with alternating background
pub fn property_row(ui: &mut egui::Ui, row_index: usize, add_contents: impl FnOnce(&mut egui::Ui)) {
    let bg_color = if row_index % 2 == 0 { ROW_BG_EVEN } else { ROW_BG_ODD };
    let available_width = ui.available_width();

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 12.0);
            add_contents(ui);
        });
}

/// Category accent colors for different component types
struct CategoryStyle {
    accent_color: Color32,
    header_bg: Color32,
}

impl CategoryStyle {
    fn transform() -> Self {
        Self {
            accent_color: Color32::from_rgb(99, 178, 238),   // Blue
            header_bg: Color32::from_rgb(35, 45, 55),
        }
    }

    fn environment() -> Self {
        Self {
            accent_color: Color32::from_rgb(134, 188, 126),  // Green
            header_bg: Color32::from_rgb(35, 48, 42),
        }
    }

    fn light() -> Self {
        Self {
            accent_color: Color32::from_rgb(247, 207, 100),  // Yellow/Gold
            header_bg: Color32::from_rgb(50, 45, 35),
        }
    }

    fn camera() -> Self {
        Self {
            accent_color: Color32::from_rgb(178, 132, 209),  // Purple
            header_bg: Color32::from_rgb(42, 38, 52),
        }
    }

    fn script() -> Self {
        Self {
            accent_color: Color32::from_rgb(236, 154, 120),  // Orange
            header_bg: Color32::from_rgb(50, 40, 38),
        }
    }

    fn physics() -> Self {
        Self {
            accent_color: Color32::from_rgb(120, 200, 200),  // Cyan
            header_bg: Color32::from_rgb(35, 48, 50),
        }
    }

    fn plugin() -> Self {
        Self {
            accent_color: Color32::from_rgb(180, 140, 180),  // Magenta
            header_bg: Color32::from_rgb(45, 38, 45),
        }
    }

    fn nodes2d() -> Self {
        Self {
            accent_color: Color32::from_rgb(242, 140, 191),  // Pink
            header_bg: Color32::from_rgb(50, 38, 45),
        }
    }

    fn ui() -> Self {
        Self {
            accent_color: Color32::from_rgb(191, 166, 242),  // Light purple
            header_bg: Color32::from_rgb(42, 40, 52),
        }
    }
}

/// Renders a styled inspector category with header and content
fn render_category(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    style: CategoryStyle,
    id_source: &str,
    default_open: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let id = ui.make_persistent_id(id_source);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);

    ui.scope(|ui| {
        // Outer frame for the entire category
        egui::Frame::new()
            .fill(Color32::from_rgb(30, 32, 36))
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {

                // Header bar
                let header_rect = ui.scope(|ui| {
                    egui::Frame::new()
                        .fill(style.header_bg)
                        .corner_radius(CornerRadius {
                            nw: 6,
                            ne: 6,
                            sw: if state.is_open() { 0 } else { 6 },
                            se: if state.is_open() { 0 } else { 6 },
                        })
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Collapse indicator
                                let caret = if state.is_open() { CARET_DOWN } else { CARET_RIGHT };
                                ui.label(RichText::new(caret).size(12.0).color(Color32::from_rgb(140, 142, 148)));

                                // Icon
                                ui.label(RichText::new(icon).size(15.0).color(style.accent_color));

                                ui.add_space(4.0);

                                // Label
                                ui.label(RichText::new(label).size(13.0).strong().color(Color32::from_rgb(220, 222, 228)));

                                // Fill remaining width
                                ui.allocate_space(ui.available_size());
                            });
                        });
                }).response.rect;

                // Make header clickable
                let header_response = ui.interact(header_rect, id.with("header"), egui::Sense::click());
                if header_response.clicked() {
                    state.toggle(ui);
                }

                // Content area with padding
                if state.is_open() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin { left: 4, right: 4, top: 0, bottom: 4 })
                        .show(ui, |ui| {
                            add_contents(ui);
                        });
                }
            });
    });

    state.store(ui.ctx());

    ui.add_space(6.0);
}

/// Renders a styled inspector category with header, content, and optional remove button
/// Returns true if the remove button was clicked
fn render_category_removable(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    style: CategoryStyle,
    id_source: &str,
    default_open: bool,
    can_remove: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) -> bool {
    let id = ui.make_persistent_id(id_source);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    let mut remove_clicked = false;

    ui.scope(|ui| {
        // Outer frame for the entire category
        egui::Frame::new()
            .fill(Color32::from_rgb(30, 32, 36))
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {

                // Header bar
                let header_rect = ui.scope(|ui| {
                    egui::Frame::new()
                        .fill(style.header_bg)
                        .corner_radius(CornerRadius {
                            nw: 6,
                            ne: 6,
                            sw: if state.is_open() { 0 } else { 6 },
                            se: if state.is_open() { 0 } else { 6 },
                        })
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Collapse indicator
                                let caret = if state.is_open() { CARET_DOWN } else { CARET_RIGHT };
                                ui.label(RichText::new(caret).size(12.0).color(Color32::from_rgb(140, 142, 148)));

                                // Icon
                                ui.label(RichText::new(icon).size(15.0).color(style.accent_color));

                                ui.add_space(4.0);

                                // Label
                                ui.label(RichText::new(label).size(13.0).strong().color(Color32::from_rgb(220, 222, 228)));

                                // Remove button on the right
                                if can_remove {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(
                                            egui::Button::new(RichText::new(X).size(11.0).color(Color32::from_rgb(180, 100, 100)))
                                                .frame(false)
                                        ).on_hover_text("Remove component").clicked() {
                                            remove_clicked = true;
                                        }
                                    });
                                } else {
                                    // Fill remaining width
                                    ui.allocate_space(ui.available_size());
                                }
                            });
                        });
                }).response.rect;

                // Make header clickable (but not the remove button area)
                let header_response = ui.interact(header_rect, id.with("header"), egui::Sense::click());
                if header_response.clicked() && !remove_clicked {
                    state.toggle(ui);
                }

                // Content area with padding
                if state.is_open() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin { left: 4, right: 4, top: 0, bottom: 4 })
                        .show(ui, |ui| {
                            add_contents(ui);
                        });
                }
            });
    });

    state.store(ui.ctx());

    ui.add_space(6.0);

    remove_clicked
}

/// System parameter that bundles all inspector-related queries
#[derive(SystemParam)]
pub struct InspectorQueries<'w, 's> {
    pub transforms: Query<'w, 's, &'static mut Transform>,
    pub world_environments: Query<'w, 's, &'static mut WorldEnvironmentMarker>,
    pub point_lights: Query<'w, 's, &'static mut PointLight>,
    pub directional_lights: Query<'w, 's, &'static mut DirectionalLight>,
    pub spot_lights: Query<'w, 's, &'static mut SpotLight>,
    pub scripts: Query<'w, 's, &'static mut ScriptComponent>,
    pub cameras: Query<'w, 's, &'static mut CameraNodeData>,
    pub camera_rigs: Query<'w, 's, &'static mut CameraRigData>,
    pub physics_bodies: Query<'w, 's, &'static mut PhysicsBodyData>,
    pub collision_shapes: Query<'w, 's, &'static mut CollisionShapeData>,
    // 2D components
    pub sprites2d: Query<'w, 's, &'static mut Sprite2DData>,
    pub cameras2d: Query<'w, 's, &'static mut Camera2DData>,
    // UI components
    pub ui_panels: Query<'w, 's, &'static mut UIPanelData>,
    pub ui_labels: Query<'w, 's, &'static mut UILabelData>,
    pub ui_buttons: Query<'w, 's, &'static mut UIButtonData>,
    pub ui_images: Query<'w, 's, &'static mut UIImageData>,
}

pub fn render_inspector(
    ctx: &egui::Context,
    selection: &SelectionState,
    entities: &Query<(Entity, &EditorEntity)>,
    queries: &mut InspectorQueries,
    script_registry: &ScriptRegistry,
    rhai_engine: &RhaiScriptEngine,
    stored_width: f32,
    camera_preview_texture_id: Option<TextureId>,
    plugin_host: &PluginHost,
    ui_renderer: &mut UiRenderer,
    component_registry: &ComponentRegistry,
    add_component_popup: &mut AddComponentPopupState,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    gizmo_state: &mut GizmoState,
) -> (Vec<UiEvent>, f32, bool) {
    let mut ui_events = Vec::new();
    let mut actual_width = stored_width;
    let mut scene_changed = false;

    // Get plugin tabs for right panel
    let api = plugin_host.api();
    let plugin_tabs = api.get_tabs_for_location(TabLocation::Right);
    let active_plugin_tab = api.get_active_tab(TabLocation::Right);

    // Calculate max width based on screen size (max 500px to match load-time clamping)
    let screen_width = ctx.screen_rect().width();
    let min_viewport_width = 200.0;
    let max_width = ((screen_width - min_viewport_width) / 2.0).max(100.0).min(500.0);
    let display_width = stored_width.clamp(100.0, max_width);
    actual_width = display_width;

    egui::SidePanel::right("inspector")
        .exact_width(display_width)
        .resizable(false)
        .frame(egui::Frame::new().fill(Color32::from_rgb(30, 32, 36)).inner_margin(egui::Margin::ZERO))
        .show(ctx, |ui| {

            // Render tab bar if there are plugin tabs
            if !plugin_tabs.is_empty() {
                ui.horizontal(|ui| {
                    // Built-in Inspector tab
                    let inspector_selected = active_plugin_tab.is_none();
                    if ui.selectable_label(inspector_selected, RichText::new(format!("{} Inspector", SLIDERS_HORIZONTAL)).size(12.0)).clicked() {
                        ui_events.push(UiEvent::PanelTabSelected { location: 1, tab_id: String::new() });
                    }

                    // Plugin tabs
                    for tab in &plugin_tabs {
                        let is_selected = active_plugin_tab == Some(tab.id.as_str());
                        let tab_label = if let Some(icon) = &tab.icon {
                            format!("{} {}", icon, tab.title)
                        } else {
                            tab.title.clone()
                        };
                        if ui.selectable_label(is_selected, RichText::new(&tab_label).size(12.0)).clicked() {
                            ui_events.push(UiEvent::PanelTabSelected { location: 1, tab_id: tab.id.clone() });
                        }
                    }
                });
                ui.separator();
            }

            // Render content based on active tab
            if let Some(tab_id) = active_plugin_tab {
                // Render plugin tab content
                if let Some(widgets) = api.get_tab_content(tab_id) {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for widget in widgets {
                            ui_renderer.render(ui, widget);
                        }
                    });
                } else {
                    ui.label(RichText::new("No content").color(Color32::GRAY));
                }
            } else {
                // Render normal inspector
                let (events, changed) = render_inspector_content(
                    ui, selection, entities, queries, script_registry, rhai_engine,
                    camera_preview_texture_id, plugin_host, ui_renderer,
                    component_registry, add_component_popup, commands, meshes, materials,
                    gizmo_state,
                );
                ui_events.extend(events);
                scene_changed = changed;
            }
        });

    // Custom resize handle at the left edge of the panel (full height)
    let resize_x = screen_width - display_width - 3.0;
    let resize_height = ctx.screen_rect().height();

    egui::Area::new(egui::Id::new("inspector_resize"))
        .fixed_pos(egui::Pos2::new(resize_x, 0.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let (resize_rect, resize_response) = ui.allocate_exact_size(
                Vec2::new(6.0, resize_height),
                egui::Sense::drag(),
            );

            if resize_response.hovered() || resize_response.dragged() {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }

            // Use pointer position for smooth resizing
            if resize_response.dragged() {
                if let Some(pointer_pos) = ctx.pointer_interact_pos() {
                    let new_width = screen_width - pointer_pos.x;
                    actual_width = new_width.clamp(100.0, max_width);
                }
            }

            // Invisible resize handle - just show cursor change
            let _ = resize_rect; // Still need the rect for interaction
        });

    (ui_events, actual_width, scene_changed)
}

/// Render inspector content (for use in docking)
/// Returns (ui_events, scene_changed)
pub fn render_inspector_content(
    ui: &mut egui::Ui,
    selection: &SelectionState,
    entities: &Query<(Entity, &EditorEntity)>,
    queries: &mut InspectorQueries,
    script_registry: &ScriptRegistry,
    rhai_engine: &RhaiScriptEngine,
    camera_preview_texture_id: Option<TextureId>,
    plugin_host: &PluginHost,
    ui_renderer: &mut UiRenderer,
    component_registry: &ComponentRegistry,
    add_component_popup: &mut AddComponentPopupState,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    gizmo_state: &mut GizmoState,
) -> (Vec<UiEvent>, bool) {
    let panel_width = ui.available_width();
    let mut scene_changed = false;
    let mut component_to_add: Option<&'static str> = None;
    let mut component_to_remove: Option<&'static str> = None;

    // Panel bar
    render_panel_bar(ui, SLIDERS, "Inspector");

    // Content area with padding
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {

    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(selected) = selection.selected_entity {
            if let Ok((_, editor_entity)) = entities.get(selected) {
                // Show multi-selection indicator if applicable
                let multi_count = selection.multi_selection.len();
                if multi_count > 1 {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} items selected", multi_count))
                            .color(Color32::from_rgb(140, 191, 242)));
                    });
                    ui.add_space(4.0);
                }

                // Entity name with accent bar
                ui.horizontal(|ui| {
                    // Accent bar
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(66, 150, 250));

                    ui.label(RichText::new(&editor_entity.name).size(16.0).strong());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(CHECK_CIRCLE).color(Color32::from_rgb(89, 191, 115)));
                        ui.label(RichText::new("Active").color(Color32::from_rgb(89, 191, 115)));
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Transform section
                if let Ok(mut transform) = queries.transforms.get_mut(selected) {
                    render_category(
                        ui,
                        ARROWS_OUT_CARDINAL,
                        "Transform",
                        CategoryStyle::transform(),
                        "inspector_transform",
                        true,
                        |ui| {
                            if render_transform_inspector(ui, &mut transform) {
                                scene_changed = true;
                            }
                        },
                    );
                }

                // World Environment
                if let Ok(mut world_env) = queries.world_environments.get_mut(selected) {
                    render_category(
                        ui,
                        GLOBE,
                        "World Environment",
                        CategoryStyle::environment(),
                        "inspector_world_env",
                        true,
                        |ui| {
                            render_world_environment_inspector(ui, &mut world_env);
                        },
                    );
                }

                // Lights (with remove buttons)
                if let Ok(mut point_light) = queries.point_lights.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        LIGHTBULB,
                        "Point Light",
                        CategoryStyle::light(),
                        "inspector_point_light",
                        true,
                        true, // can_remove
                        |ui| {
                            render_point_light_inspector(ui, &mut point_light);
                        },
                    ) {
                        component_to_remove = Some("point_light");
                    }
                }

                if let Ok(mut dir_light) = queries.directional_lights.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        SUN,
                        "Directional Light",
                        CategoryStyle::light(),
                        "inspector_dir_light",
                        true,
                        true, // can_remove
                        |ui| {
                            render_directional_light_inspector(ui, &mut dir_light);
                        },
                    ) {
                        component_to_remove = Some("directional_light");
                    }
                }

                if let Ok(mut spot_light) = queries.spot_lights.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        FLASHLIGHT,
                        "Spot Light",
                        CategoryStyle::light(),
                        "inspector_spot_light",
                        true,
                        true, // can_remove
                        |ui| {
                            render_spot_light_inspector(ui, &mut spot_light);
                        },
                    ) {
                        component_to_remove = Some("spot_light");
                    }
                }

                // Camera component (with remove button)
                if let Ok(mut camera_data) = queries.cameras.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        VIDEO_CAMERA,
                        "Camera3D",
                        CategoryStyle::camera(),
                        "inspector_camera",
                        true,
                        true, // can_remove
                        |ui| {
                            render_camera_inspector(ui, &mut camera_data, camera_preview_texture_id);
                        },
                    ) {
                        component_to_remove = Some("camera_3d");
                    }
                }

                // Camera rig component (with remove button)
                if let Ok(mut rig_data) = queries.camera_rigs.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        VIDEO_CAMERA,
                        "Camera Rig",
                        CategoryStyle::camera(),
                        "inspector_camera_rig",
                        true,
                        true, // can_remove
                        |ui| {
                            if render_camera_rig_inspector(ui, &mut rig_data, camera_preview_texture_id) {
                                scene_changed = true;
                            }
                        },
                    ) {
                        component_to_remove = Some("camera_rig");
                    }
                }

                // Script component (with remove button)
                if let Ok(mut script) = queries.scripts.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        CODE,
                        "Script",
                        CategoryStyle::script(),
                        "inspector_script",
                        true,
                        true, // can_remove
                        |ui| {
                            render_script_inspector(ui, &mut script, script_registry, rhai_engine);
                        },
                    ) {
                        component_to_remove = Some("script");
                    }
                }

                // Physics body (with remove button)
                if let Ok(mut physics_body) = queries.physics_bodies.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        ATOM,
                        "Rigid Body",
                        CategoryStyle::physics(),
                        "inspector_physics_body",
                        true,
                        true, // can_remove
                        |ui| {
                            render_physics_body_inspector(ui, &mut physics_body);
                        },
                    ) {
                        component_to_remove = Some("rigid_body");
                    }
                }

                // Collision shape (with remove button)
                if let Ok(mut collision_shape) = queries.collision_shapes.get_mut(selected) {
                    // Determine which collider type to remove based on shape_type
                    let collider_type = match collision_shape.shape_type {
                        crate::shared::CollisionShapeType::Box => "box_collider",
                        crate::shared::CollisionShapeType::Sphere => "sphere_collider",
                        crate::shared::CollisionShapeType::Capsule => "capsule_collider",
                        crate::shared::CollisionShapeType::Cylinder => "box_collider", // fallback
                    };
                    if render_category_removable(
                        ui,
                        CUBE,
                        "Collider",
                        CategoryStyle::physics(),
                        "inspector_collision_shape",
                        true,
                        true, // can_remove
                        |ui| {
                            render_collision_shape_inspector(ui, &mut collision_shape, selected, gizmo_state);
                        },
                    ) {
                        component_to_remove = Some(collider_type);
                    }
                }

                // 2D Sprite (with remove button)
                if let Ok(mut sprite_data) = queries.sprites2d.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        IMAGE,
                        "Sprite2D",
                        CategoryStyle::nodes2d(),
                        "inspector_sprite2d",
                        true,
                        true, // can_remove
                        |ui| {
                            if render_sprite2d_inspector(ui, &mut sprite_data) {
                                scene_changed = true;
                            }
                        },
                    ) {
                        component_to_remove = Some("sprite_2d");
                    }
                }

                // 2D Camera (with remove button)
                if let Ok(mut camera_data) = queries.cameras2d.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        VIDEO_CAMERA,
                        "Camera2D",
                        CategoryStyle::nodes2d(),
                        "inspector_camera2d",
                        true,
                        true, // can_remove
                        |ui| {
                            if render_camera2d_inspector(ui, &mut camera_data) {
                                scene_changed = true;
                            }
                        },
                    ) {
                        component_to_remove = Some("camera_2d");
                    }
                }

                // UI Panel (with remove button)
                if let Ok(mut panel_data) = queries.ui_panels.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        STACK,
                        "UI Panel",
                        CategoryStyle::ui(),
                        "inspector_ui_panel",
                        true,
                        true, // can_remove
                        |ui| {
                            if render_ui_panel_inspector(ui, &mut panel_data) {
                                scene_changed = true;
                            }
                        },
                    ) {
                        component_to_remove = Some("ui_panel");
                    }
                }

                // UI Label (with remove button)
                if let Ok(mut label_data) = queries.ui_labels.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        TEXTBOX,
                        "UI Label",
                        CategoryStyle::ui(),
                        "inspector_ui_label",
                        true,
                        true, // can_remove
                        |ui| {
                            if render_ui_label_inspector(ui, &mut label_data) {
                                scene_changed = true;
                            }
                        },
                    ) {
                        component_to_remove = Some("ui_label");
                    }
                }

                // UI Button (with remove button)
                if let Ok(mut button_data) = queries.ui_buttons.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        CURSOR_CLICK,
                        "UI Button",
                        CategoryStyle::ui(),
                        "inspector_ui_button",
                        true,
                        true, // can_remove
                        |ui| {
                            if render_ui_button_inspector(ui, &mut button_data) {
                                scene_changed = true;
                            }
                        },
                    ) {
                        component_to_remove = Some("ui_button");
                    }
                }

                // UI Image (with remove button)
                if let Ok(mut image_data) = queries.ui_images.get_mut(selected) {
                    if render_category_removable(
                        ui,
                        IMAGE,
                        "UI Image",
                        CategoryStyle::ui(),
                        "inspector_ui_image",
                        true,
                        true, // can_remove
                        |ui| {
                            if render_ui_image_inspector(ui, &mut image_data) {
                                scene_changed = true;
                            }
                        },
                    ) {
                        component_to_remove = Some("ui_image");
                    }
                }

                // Plugin-registered inspector sections
                let api = plugin_host.api();
                for (type_id, inspector_def, _plugin_id) in &api.inspectors {
                    if let Some(content) = api.inspector_contents.get(type_id) {
                        render_category(
                            ui,
                            PUZZLE_PIECE,
                            &inspector_def.label,
                            CategoryStyle::plugin(),
                            &format!("inspector_plugin_{:?}", type_id),
                            true,
                            |ui| {
                                for widget in content {
                                    ui_renderer.render(ui, widget);
                                }
                            },
                        );
                    }
                }

                ui.add_space(16.0);

                // Add Component button and popup
                component_to_add = render_add_component_popup(
                    ui,
                    selected,
                    component_registry,
                    add_component_popup,
                    panel_width,
                );
            }
        } else {
            // Close popup when nothing is selected
            add_component_popup.is_open = false;

            // No selection state
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(MAGNIFYING_GLASS).size(32.0).color(Color32::from_rgb(80, 80, 90)));
                ui.add_space(8.0);
                ui.label(RichText::new("No Selection").weak());
                ui.add_space(8.0);
                ui.label(RichText::new("Select an entity in the Hierarchy\nor click on an object in the Scene.").weak());
            });
        }
    });

    }); // End content frame

    // Handle component addition
    if let (Some(type_id), Some(selected)) = (component_to_add, selection.selected_entity) {
        if let Some(def) = component_registry.get(type_id) {
            (def.add_fn)(commands, selected, meshes, materials);
            scene_changed = true;
        }
    }

    // Handle component removal
    if let (Some(type_id), Some(selected)) = (component_to_remove, selection.selected_entity) {
        if let Some(def) = component_registry.get(type_id) {
            (def.remove_fn)(commands, selected);
            scene_changed = true;
        }
    }

    // Collect events from ui_renderer
    (ui_renderer.drain_events().collect(), scene_changed)
}

/// Render the Add Component button and popup
fn render_add_component_popup(
    ui: &mut egui::Ui,
    _entity: Entity,
    registry: &ComponentRegistry,
    popup_state: &mut AddComponentPopupState,
    panel_width: f32,
) -> Option<&'static str> {
    let mut component_to_add: Option<&'static str> = None;

    // Add Component button
    if ui
        .add_sized(
            Vec2::new(panel_width - 20.0, 28.0),
            egui::Button::new(RichText::new(format!("{} Add Component", PLUS)).color(Color32::WHITE))
                .fill(Color32::from_rgb(50, 70, 100)),
        )
        .clicked()
    {
        popup_state.is_open = !popup_state.is_open;
        popup_state.search_text.clear();
        popup_state.selected_category = None;
    }

    // Popup
    if popup_state.is_open {
        ui.add_space(4.0);

        egui::Frame::new()
            .fill(Color32::from_rgb(35, 37, 42))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(55, 57, 62)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                // Search bar
                ui.horizontal(|ui| {
                    ui.label(RichText::new(MAGNIFYING_GLASS).color(Color32::GRAY));
                    ui.add(
                        egui::TextEdit::singleline(&mut popup_state.search_text)
                            .hint_text("Search components...")
                            .desired_width(ui.available_width() - 40.0),
                    );
                    if ui
                        .add(egui::Button::new(RichText::new(X).color(Color32::GRAY)).frame(false))
                        .clicked()
                    {
                        popup_state.is_open = false;
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Filter by search text
                let search_lower = popup_state.search_text.to_lowercase();

                // Get all available components
                let all_components: Vec<_> = registry.all().collect();

                // Filter
                let filtered: Vec<_> = if search_lower.is_empty() {
                    all_components.clone()
                } else {
                    all_components
                        .iter()
                        .filter(|def| {
                            def.display_name.to_lowercase().contains(&search_lower)
                                || def.type_id.contains(&search_lower)
                        })
                        .copied()
                        .collect()
                };

                // Show by category or flat list if searching
                if search_lower.is_empty() {
                    // Show categorized view
                    for category in ComponentCategory::all_in_order() {
                        let in_category: Vec<_> = filtered
                            .iter()
                            .filter(|d| d.category == *category)
                            .copied()
                            .collect();

                        if !in_category.is_empty() {
                            let (accent, _header_bg) = get_category_style(*category);
                            egui::CollapsingHeader::new(
                                RichText::new(format!("{} {}", category.icon(), category.display_name()))
                                    .color(accent),
                            )
                            .default_open(false)
                            .show(ui, |ui| {
                                for def in in_category {
                                    if render_component_menu_item(ui, def) {
                                        component_to_add = Some(def.type_id);
                                        popup_state.is_open = false;
                                    }
                                }
                            });
                        }
                    }
                } else {
                    // Flat search results
                    if filtered.is_empty() {
                        ui.label(
                            RichText::new("No matching components")
                                .color(Color32::GRAY)
                                .italics(),
                        );
                    } else {
                        for def in &filtered {
                            if render_component_menu_item(ui, def) {
                                component_to_add = Some(def.type_id);
                                popup_state.is_open = false;
                            }
                        }
                    }
                }
            });
    }

    component_to_add
}

/// Render a single component item in the Add Component popup
fn render_component_menu_item(
    ui: &mut egui::Ui,
    def: &crate::component_system::ComponentDefinition,
) -> bool {
    let response = ui.add_sized(
        Vec2::new(ui.available_width(), 24.0),
        egui::Button::new(
            RichText::new(format!("{} {}", def.icon, def.display_name))
                .color(Color32::from_rgb(200, 200, 210)),
        )
        .fill(Color32::TRANSPARENT)
        .frame(false),
    );

    if response.hovered() {
        ui.painter().rect_filled(
            response.rect,
            3.0,
            Color32::from_rgb(55, 60, 70),
        );
    }

    response.clicked()
}
