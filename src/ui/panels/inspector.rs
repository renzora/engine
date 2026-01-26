#![allow(dead_code)]

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, RichText, Sense, TextureId, Vec2};

use crate::component_system::{
    AddComponentPopupState, ComponentCategory, ComponentRegistry,
    get_category_style,
};
use crate::commands::CommandHistory;
use crate::core::{EditorEntity, RightPanelTab, SelectionState, WorldEnvironmentMarker};
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

// Icon for inspector tab
use egui_phosphor::regular::{SLIDERS_HORIZONTAL, CLOCK_COUNTER_CLOCKWISE};

// Phosphor icons for inspector
use egui_phosphor::regular::{
    ARROWS_OUT_CARDINAL, GLOBE, LIGHTBULB, SUN, FLASHLIGHT,
    PLUS, MAGNIFYING_GLASS, CHECK_CIRCLE, CODE, VIDEO_CAMERA, PUZZLE_PIECE,
    CUBE, ATOM, CARET_DOWN, CARET_RIGHT, IMAGE, STACK, TEXTBOX, CURSOR_CLICK, X,
    TAG, PENCIL_SIMPLE,
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
    command_history: &mut CommandHistory,
    right_panel_tab: &mut RightPanelTab,
) -> (Vec<UiEvent>, f32, bool) {
    let mut ui_events = Vec::new();
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
    let mut actual_width = display_width;

    egui::SidePanel::right("inspector")
        .exact_width(display_width)
        .resizable(false)
        .frame(egui::Frame::new().fill(Color32::from_rgb(30, 32, 36)).inner_margin(egui::Margin::ZERO))
        .show(ctx, |ui| {

            // Tab bar styled like bottom panel
            let bar_height = 24.0;
            let available_width = ui.available_width();
            let (bar_rect, _) = ui.allocate_exact_size(
                Vec2::new(available_width, bar_height),
                Sense::hover(),
            );

            // Draw bar background
            ui.painter().rect_filled(
                bar_rect,
                CornerRadius::ZERO,
                Color32::from_rgb(38, 40, 46),
            );

            // Draw bottom border
            ui.painter().line_segment(
                [
                    egui::pos2(bar_rect.min.x, bar_rect.max.y),
                    egui::pos2(bar_rect.max.x, bar_rect.max.y),
                ],
                egui::Stroke::new(1.0, Color32::from_rgb(50, 52, 58)),
            );

            // Draw tabs inside the bar
            let mut tab_x = bar_rect.min.x + 8.0;
            let tab_y = bar_rect.min.y;
            let tab_height = bar_height;

            // Inspector tab
            let inspector_selected = *right_panel_tab == RightPanelTab::Inspector && active_plugin_tab.is_none();
            let inspector_text = format!("{} Inspector", SLIDERS_HORIZONTAL);
            let inspector_width = 80.0;
            let inspector_rect = egui::Rect::from_min_size(
                egui::pos2(tab_x, tab_y),
                Vec2::new(inspector_width, tab_height),
            );

            let inspector_response = ui.interact(inspector_rect, ui.id().with("inspector_tab"), Sense::click());
            if inspector_response.clicked() {
                *right_panel_tab = RightPanelTab::Inspector;
                ui_events.push(UiEvent::PanelTabSelected { location: 1, tab_id: String::new() });
            }

            let inspector_bg = if inspector_selected {
                Color32::from_rgb(50, 52, 60)
            } else if inspector_response.hovered() {
                Color32::from_rgb(45, 47, 55)
            } else {
                Color32::TRANSPARENT
            };
            if inspector_bg != Color32::TRANSPARENT {
                ui.painter().rect_filled(inspector_rect, 0.0, inspector_bg);
            }
            ui.painter().text(
                inspector_rect.center(),
                egui::Align2::CENTER_CENTER,
                &inspector_text,
                egui::FontId::proportional(12.0),
                if inspector_selected { Color32::WHITE } else { Color32::from_rgb(160, 162, 170) },
            );

            tab_x += inspector_width + 4.0;

            // History tab
            let history_selected = *right_panel_tab == RightPanelTab::History && active_plugin_tab.is_none();
            let undo_count = command_history.undo_count();
            let history_text = if undo_count > 0 {
                format!("{} History ({})", CLOCK_COUNTER_CLOCKWISE, undo_count)
            } else {
                format!("{} History", CLOCK_COUNTER_CLOCKWISE)
            };
            let history_width = if undo_count > 0 { 90.0 } else { 70.0 };
            let history_rect = egui::Rect::from_min_size(
                egui::pos2(tab_x, tab_y),
                Vec2::new(history_width, tab_height),
            );

            let history_response = ui.interact(history_rect, ui.id().with("history_tab"), Sense::click());
            if history_response.clicked() {
                *right_panel_tab = RightPanelTab::History;
                ui_events.push(UiEvent::PanelTabSelected { location: 1, tab_id: String::new() });
            }

            let history_bg = if history_selected {
                Color32::from_rgb(50, 52, 60)
            } else if history_response.hovered() {
                Color32::from_rgb(45, 47, 55)
            } else {
                Color32::TRANSPARENT
            };
            if history_bg != Color32::TRANSPARENT {
                ui.painter().rect_filled(history_rect, 0.0, history_bg);
            }
            ui.painter().text(
                history_rect.center(),
                egui::Align2::CENTER_CENTER,
                &history_text,
                egui::FontId::proportional(12.0),
                if history_selected { Color32::WHITE } else { Color32::from_rgb(160, 162, 170) },
            );

            tab_x += history_width + 4.0;

            // Plugin tabs
            for tab in &plugin_tabs {
                let is_selected = active_plugin_tab == Some(tab.id.as_str());
                let tab_icon = tab.icon.as_deref().unwrap_or("");
                let tab_text = format!("{} {}", tab_icon, tab.title);
                let plugin_tab_width = 80.0;
                let plugin_rect = egui::Rect::from_min_size(
                    egui::pos2(tab_x, tab_y),
                    Vec2::new(plugin_tab_width, tab_height),
                );

                let plugin_response = ui.interact(plugin_rect, ui.id().with(&tab.id), Sense::click());
                if plugin_response.clicked() {
                    ui_events.push(UiEvent::PanelTabSelected { location: 1, tab_id: tab.id.clone() });
                }

                let plugin_bg = if is_selected {
                    Color32::from_rgb(50, 52, 60)
                } else if plugin_response.hovered() {
                    Color32::from_rgb(45, 47, 55)
                } else {
                    Color32::TRANSPARENT
                };
                if plugin_bg != Color32::TRANSPARENT {
                    ui.painter().rect_filled(plugin_rect, 0.0, plugin_bg);
                }
                ui.painter().text(
                    plugin_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &tab_text,
                    egui::FontId::proportional(12.0),
                    if is_selected { Color32::WHITE } else { Color32::from_rgb(160, 162, 170) },
                );

                tab_x += plugin_tab_width + 4.0;
            }
            let _ = tab_x; // Suppress unused warning

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
            } else if *right_panel_tab == RightPanelTab::History {
                // Render history tab
                render_history_content(ui, command_history);
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
    let mut scene_changed = false;
    let mut component_to_remove: Option<&'static str> = None;

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

                // Clone editor entity data upfront to avoid borrow issues
                let mut current_name = editor_entity.name.clone();
                let mut current_tag = editor_entity.tag.clone();
                let current_visible = editor_entity.visible;
                let current_locked = editor_entity.locked;

                // Entity header with editable name
                let mut name_changed = false;
                let mut tag_changed = false;

                ui.horizontal(|ui| {
                    // Accent bar
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(66, 150, 250));

                    ui.label(RichText::new(PENCIL_SIMPLE).size(14.0).color(Color32::from_rgb(140, 142, 148)));

                    // Editable entity name
                    let name_response = ui.add(
                        egui::TextEdit::singleline(&mut current_name)
                            .desired_width(ui.available_width() - 80.0)
                            .font(egui::TextStyle::Heading)
                    );
                    if name_response.changed() && !current_name.is_empty() {
                        name_changed = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Active indicator
                        ui.label(RichText::new(CHECK_CIRCLE).color(Color32::from_rgb(89, 191, 115)));
                    });
                });

                ui.add_space(4.0);

                // Tag input
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(TAG).size(14.0).color(Color32::from_rgb(140, 142, 148)));
                    ui.label(RichText::new("Tag").size(12.0).color(Color32::from_rgb(140, 142, 148)));

                    let tag_response = ui.add(
                        egui::TextEdit::singleline(&mut current_tag)
                            .desired_width(ui.available_width() - 10.0)
                            .hint_text("Untagged")
                    );
                    if tag_response.changed() {
                        tag_changed = true;
                    }
                });

                // Apply changes after UI code completes
                if name_changed || tag_changed {
                    commands.entity(selected).insert(EditorEntity {
                        name: current_name,
                        tag: current_tag,
                        visible: current_visible,
                        locked: current_locked,
                    });
                    scene_changed = true;
                }

                ui.add_space(8.0);

                // Add Component dropdown button (full width)
                egui::Frame::default()
                    .fill(Color32::from_rgb(40, 45, 55))
                    .corner_radius(CornerRadius::same(4))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width() - 12.0);
                        ui.menu_button(
                            RichText::new(format!("{} Add Component", PLUS)).color(Color32::from_rgb(100, 180, 255)),
                            |ui| {
                                ui.set_min_width(220.0);

                                for category in ComponentCategory::all_in_order() {
                                    let in_category: Vec<_> = component_registry
                                        .all()
                                        .filter(|d| d.category == *category)
                                        .collect();

                                    if !in_category.is_empty() {
                                        let (accent, _header_bg) = get_category_style(*category);
                                        let label = format!("{} {}", category.icon(), category.display_name());

                                        ui.menu_button(RichText::new(label).color(accent), |ui| {
                                            ui.set_min_width(180.0);

                                            for def in in_category {
                                                let item_label = format!("{} {}", def.icon, def.display_name);
                                                if ui.button(RichText::new(item_label)).clicked() {
                                                    (def.add_fn)(commands, selected, meshes, materials);
                                                    scene_changed = true;
                                                    ui.close();
                                                }
                                            }
                                        });
                                    }
                                }
                            },
                        );
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

/// Render the history panel content
pub fn render_history_content(
    ui: &mut egui::Ui,
    command_history: &mut CommandHistory,
) {
    use egui_phosphor::regular::{ARROW_U_UP_LEFT, ARROW_U_UP_RIGHT, CIRCLE};

    // Content area with padding
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let undo_descriptions = command_history.undo_descriptions();
                let redo_descriptions = command_history.redo_descriptions();
                let undo_count = undo_descriptions.len();
                let redo_count = redo_descriptions.len();

                if undo_descriptions.is_empty() && redo_descriptions.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(CLOCK_COUNTER_CLOCKWISE).size(32.0).color(Color32::from_rgb(80, 80, 90)));
                        ui.add_space(8.0);
                        ui.label(RichText::new("No History").weak());
                        ui.add_space(8.0);
                        ui.label(RichText::new("Actions you perform will\nappear here.").weak());
                    });
                    return;
                }

                // Show redo stack (future actions) - displayed at top, grayed out
                // Clicking on a redo item will redo all actions up to and including that item
                if !redo_descriptions.is_empty() {
                    ui.label(RichText::new("Redo Stack").size(11.0).color(Color32::from_rgb(100, 100, 110)));
                    ui.add_space(4.0);

                    // Show redo items in reverse order (most recent undone at top)
                    // Index 0 in reversed = last item in original = furthest from current state
                    for (display_i, desc) in redo_descriptions.iter().rev().enumerate() {
                        // Number of redos needed: redo_count - display_i (to reach this state)
                        let redos_needed = redo_count - display_i;

                        let row_id = ui.id().with(("redo", display_i));
                        let available_width = ui.available_width();
                        let (row_rect, row_response) = ui.allocate_exact_size(
                            Vec2::new(available_width, 24.0),
                            Sense::click(),
                        );

                        let is_hovered = row_response.hovered();
                        let bg_color = if is_hovered {
                            Color32::from_rgb(50, 55, 65)
                        } else if display_i % 2 == 0 {
                            ROW_BG_EVEN
                        } else {
                            ROW_BG_ODD
                        };

                        ui.painter().rect_filled(row_rect, 3.0, bg_color);

                        // Draw icon and text
                        let icon_pos = egui::pos2(row_rect.min.x + 8.0, row_rect.center().y);
                        let text_pos = egui::pos2(row_rect.min.x + 26.0, row_rect.center().y);

                        ui.painter().text(
                            icon_pos,
                            egui::Align2::LEFT_CENTER,
                            ARROW_U_UP_RIGHT,
                            egui::FontId::proportional(12.0),
                            if is_hovered { Color32::from_rgb(120, 120, 130) } else { Color32::from_rgb(80, 80, 90) },
                        );
                        ui.painter().text(
                            text_pos,
                            egui::Align2::LEFT_CENTER,
                            desc,
                            egui::FontId::proportional(12.0),
                            if is_hovered { Color32::from_rgb(140, 140, 150) } else { Color32::from_rgb(100, 100, 110) },
                        );

                        if row_response.clicked() {
                            command_history.pending_redo = redos_needed;
                        }

                        if is_hovered {
                            row_response.on_hover_text(format!("Click to redo {} action(s)", redos_needed));
                        }

                        ui.add_space(1.0);
                        let _ = row_id;
                    }
                    ui.add_space(8.0);
                }

                // Current state indicator
                ui.horizontal(|ui| {
                    ui.label(RichText::new(CIRCLE).size(10.0).color(Color32::from_rgb(89, 191, 115)));
                    ui.label(RichText::new("Current State").size(11.0).color(Color32::from_rgb(89, 191, 115)));
                });
                ui.add_space(8.0);

                // Show undo stack (past actions) - displayed below current state
                // Clicking on an undo item will undo all actions back to that point
                if !undo_descriptions.is_empty() {
                    ui.label(RichText::new("Undo Stack").size(11.0).color(Color32::from_rgb(140, 142, 148)));
                    ui.add_space(4.0);

                    // Show undo items in reverse order (most recent at top)
                    // Index 0 in reversed = last item in original = most recent = 1 undo
                    for (display_i, desc) in undo_descriptions.iter().rev().enumerate() {
                        // Number of undos needed: display_i + 1 (to reach the state BEFORE this action)
                        let undos_needed = display_i + 1;
                        let is_next = display_i == 0;

                        let row_id = ui.id().with(("undo", display_i));
                        let available_width = ui.available_width();
                        let (row_rect, row_response) = ui.allocate_exact_size(
                            Vec2::new(available_width, 24.0),
                            Sense::click(),
                        );

                        let is_hovered = row_response.hovered();
                        let bg_color = if is_hovered {
                            Color32::from_rgb(50, 55, 65)
                        } else if display_i % 2 == 0 {
                            ROW_BG_EVEN
                        } else {
                            ROW_BG_ODD
                        };

                        ui.painter().rect_filled(row_rect, 3.0, bg_color);

                        // Draw icon and text
                        let icon_pos = egui::pos2(row_rect.min.x + 8.0, row_rect.center().y);
                        let text_pos = egui::pos2(row_rect.min.x + 26.0, row_rect.center().y);

                        let icon_color = if is_hovered {
                            Color32::from_rgb(130, 200, 255)
                        } else if is_next {
                            Color32::from_rgb(99, 178, 238)
                        } else {
                            Color32::from_rgb(140, 142, 148)
                        };
                        let text_color = if is_hovered {
                            Color32::WHITE
                        } else if is_next {
                            Color32::from_rgb(220, 222, 228)
                        } else {
                            Color32::from_rgb(160, 162, 168)
                        };

                        ui.painter().text(
                            icon_pos,
                            egui::Align2::LEFT_CENTER,
                            ARROW_U_UP_LEFT,
                            egui::FontId::proportional(12.0),
                            icon_color,
                        );
                        ui.painter().text(
                            text_pos,
                            egui::Align2::LEFT_CENTER,
                            desc,
                            egui::FontId::proportional(12.0),
                            text_color,
                        );

                        if row_response.clicked() {
                            command_history.pending_undo = undos_needed;
                        }

                        if is_hovered {
                            row_response.on_hover_text(format!("Click to undo {} action(s)", undos_needed));
                        }

                        ui.add_space(1.0);
                        let _ = row_id;
                    }
                }

                ui.add_space(16.0);

                // Stats
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Undo: {}", undo_count)).size(11.0).color(Color32::from_rgb(140, 142, 148)));
                    ui.label(RichText::new("|").size(11.0).color(Color32::from_rgb(60, 62, 68)));
                    ui.label(RichText::new(format!("Redo: {}", redo_count)).size(11.0).color(Color32::from_rgb(140, 142, 148)));
                });
            });
        });
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
