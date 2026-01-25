use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, Rounding, TextureId, Vec2};

use crate::core::{EditorEntity, SelectionState, WorldEnvironmentMarker};
use crate::node_system::{
    render_camera_inspector, render_camera_rig_inspector, render_collision_shape_inspector,
    render_directional_light_inspector, render_physics_body_inspector, render_point_light_inspector,
    render_script_inspector, render_spot_light_inspector, render_transform_inspector,
    render_world_environment_inspector,
    // 2D inspectors
    render_sprite2d_inspector, render_camera2d_inspector,
    // UI inspectors
    render_ui_panel_inspector, render_ui_label_inspector, render_ui_button_inspector, render_ui_image_inspector,
    CameraNodeData, CollisionShapeData, PhysicsBodyData,
};
use crate::shared::{
    CameraRigData,
    // 2D components
    Sprite2DData, Camera2DData,
    // UI components
    UIPanelData, UILabelData, UIButtonData, UIImageData,
};
use crate::plugin_core::{PluginHost, TabLocation};
use crate::scripting::{ScriptComponent, ScriptRegistry, RhaiScriptEngine};
use crate::ui_api::{renderer::UiRenderer, UiEvent};

// Icon for inspector tab
use egui_phosphor::regular::SLIDERS_HORIZONTAL;

// Phosphor icons for inspector
use egui_phosphor::regular::{
    SLIDERS, ARROWS_OUT_CARDINAL, GLOBE, LIGHTBULB, SUN, FLASHLIGHT,
    PLUS, MAGNIFYING_GLASS, CHECK_CIRCLE, CODE, VIDEO_CAMERA, PUZZLE_PIECE,
    CUBE, ATOM, CARET_DOWN, CARET_RIGHT, IMAGE, STACK, TEXTBOX, CURSOR_CLICK,
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
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 20.0);
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
            .corner_radius(Rounding::same(6))
            .show(ui, |ui| {

                // Header bar
                let header_rect = ui.scope(|ui| {
                    egui::Frame::new()
                        .fill(style.header_bg)
                        .corner_radius(Rounding {
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
) -> (Vec<UiEvent>, f32, bool) {
    let mut ui_events = Vec::new();
    let mut actual_width = stored_width;
    let mut scene_changed = false;

    // Get plugin tabs for right panel
    let api = plugin_host.api();
    let plugin_tabs = api.get_tabs_for_location(TabLocation::Right);
    let active_plugin_tab = api.get_active_tab(TabLocation::Right);

    egui::SidePanel::right("inspector")
        .default_width(stored_width)
        .resizable(true)
        .show(ctx, |ui| {
            // Get actual width from the panel
            actual_width = ui.available_width() + 16.0; // Account for panel padding

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
                let (events, changed) = render_inspector_content(ui, selection, entities, queries, script_registry, rhai_engine, camera_preview_texture_id, plugin_host, ui_renderer);
                ui_events.extend(events);
                scene_changed = changed;
            }
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
) -> (Vec<UiEvent>, bool) {
    let panel_width = ui.available_width();
    let mut scene_changed = false;

    // Compact tab header
    ui.horizontal(|ui| {
        let tab_height = 24.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(80.0, tab_height), egui::Sense::hover());

        // Draw tab background
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 },
            Color32::from_rgb(45, 47, 53),
        );

        // Tab text
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{} Inspector", SLIDERS),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(200, 200, 210),
        );
    });

    ui.add_space(4.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(selected) = selection.selected_entity {
            if let Ok((_, editor_entity)) = entities.get(selected) {
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

                // Lights
                if let Ok(mut point_light) = queries.point_lights.get_mut(selected) {
                    render_category(
                        ui,
                        LIGHTBULB,
                        "Point Light",
                        CategoryStyle::light(),
                        "inspector_point_light",
                        true,
                        |ui| {
                            render_point_light_inspector(ui, &mut point_light);
                        },
                    );
                }

                if let Ok(mut dir_light) = queries.directional_lights.get_mut(selected) {
                    render_category(
                        ui,
                        SUN,
                        "Directional Light",
                        CategoryStyle::light(),
                        "inspector_dir_light",
                        true,
                        |ui| {
                            render_directional_light_inspector(ui, &mut dir_light);
                        },
                    );
                }

                if let Ok(mut spot_light) = queries.spot_lights.get_mut(selected) {
                    render_category(
                        ui,
                        FLASHLIGHT,
                        "Spot Light",
                        CategoryStyle::light(),
                        "inspector_spot_light",
                        true,
                        |ui| {
                            render_spot_light_inspector(ui, &mut spot_light);
                        },
                    );
                }

                // Camera component
                if let Ok(mut camera_data) = queries.cameras.get_mut(selected) {
                    render_category(
                        ui,
                        VIDEO_CAMERA,
                        "Camera3D",
                        CategoryStyle::camera(),
                        "inspector_camera",
                        true,
                        |ui| {
                            render_camera_inspector(ui, &mut camera_data, camera_preview_texture_id);
                        },
                    );
                }

                // Camera rig component
                if let Ok(mut rig_data) = queries.camera_rigs.get_mut(selected) {
                    render_category(
                        ui,
                        VIDEO_CAMERA,
                        "Camera Rig",
                        CategoryStyle::camera(),
                        "inspector_camera_rig",
                        true,
                        |ui| {
                            if render_camera_rig_inspector(ui, &mut rig_data, camera_preview_texture_id) {
                                scene_changed = true;
                            }
                        },
                    );
                }

                // Script component
                if let Ok(mut script) = queries.scripts.get_mut(selected) {
                    render_category(
                        ui,
                        CODE,
                        "Script",
                        CategoryStyle::script(),
                        "inspector_script",
                        true,
                        |ui| {
                            render_script_inspector(ui, &mut script, script_registry, rhai_engine);
                        },
                    );
                }

                // Physics body
                if let Ok(mut physics_body) = queries.physics_bodies.get_mut(selected) {
                    render_category(
                        ui,
                        ATOM,
                        "Physics Body",
                        CategoryStyle::physics(),
                        "inspector_physics_body",
                        true,
                        |ui| {
                            render_physics_body_inspector(ui, &mut physics_body);
                        },
                    );
                }

                // Collision shape
                if let Ok(mut collision_shape) = queries.collision_shapes.get_mut(selected) {
                    render_category(
                        ui,
                        CUBE,
                        "Collision Shape",
                        CategoryStyle::physics(),
                        "inspector_collision_shape",
                        true,
                        |ui| {
                            render_collision_shape_inspector(ui, &mut collision_shape);
                        },
                    );
                }

                // 2D Sprite
                if let Ok(mut sprite_data) = queries.sprites2d.get_mut(selected) {
                    render_category(
                        ui,
                        IMAGE,
                        "Sprite2D",
                        CategoryStyle::nodes2d(),
                        "inspector_sprite2d",
                        true,
                        |ui| {
                            if render_sprite2d_inspector(ui, &mut sprite_data) {
                                scene_changed = true;
                            }
                        },
                    );
                }

                // 2D Camera
                if let Ok(mut camera_data) = queries.cameras2d.get_mut(selected) {
                    render_category(
                        ui,
                        VIDEO_CAMERA,
                        "Camera2D",
                        CategoryStyle::nodes2d(),
                        "inspector_camera2d",
                        true,
                        |ui| {
                            if render_camera2d_inspector(ui, &mut camera_data) {
                                scene_changed = true;
                            }
                        },
                    );
                }

                // UI Panel
                if let Ok(mut panel_data) = queries.ui_panels.get_mut(selected) {
                    render_category(
                        ui,
                        STACK,
                        "UI Panel",
                        CategoryStyle::ui(),
                        "inspector_ui_panel",
                        true,
                        |ui| {
                            if render_ui_panel_inspector(ui, &mut panel_data) {
                                scene_changed = true;
                            }
                        },
                    );
                }

                // UI Label
                if let Ok(mut label_data) = queries.ui_labels.get_mut(selected) {
                    render_category(
                        ui,
                        TEXTBOX,
                        "UI Label",
                        CategoryStyle::ui(),
                        "inspector_ui_label",
                        true,
                        |ui| {
                            if render_ui_label_inspector(ui, &mut label_data) {
                                scene_changed = true;
                            }
                        },
                    );
                }

                // UI Button
                if let Ok(mut button_data) = queries.ui_buttons.get_mut(selected) {
                    render_category(
                        ui,
                        CURSOR_CLICK,
                        "UI Button",
                        CategoryStyle::ui(),
                        "inspector_ui_button",
                        true,
                        |ui| {
                            if render_ui_button_inspector(ui, &mut button_data) {
                                scene_changed = true;
                            }
                        },
                    );
                }

                // UI Image
                if let Ok(mut image_data) = queries.ui_images.get_mut(selected) {
                    render_category(
                        ui,
                        IMAGE,
                        "UI Image",
                        CategoryStyle::ui(),
                        "inspector_ui_image",
                        true,
                        |ui| {
                            if render_ui_image_inspector(ui, &mut image_data) {
                                scene_changed = true;
                            }
                        },
                    );
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

                // Add Component button
                if ui.add_sized(
                    Vec2::new(panel_width - 20.0, 28.0),
                    egui::Button::new(format!("{} Add Component", PLUS)),
                ).clicked() {
                    // TODO: Add component popup
                }
            }
        } else {
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

    // Collect events from ui_renderer
    (ui_renderer.drain_events().collect(), scene_changed)
}
