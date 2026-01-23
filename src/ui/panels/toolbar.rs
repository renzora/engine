use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Sense, Vec2, RichText};

use crate::core::{EditorSettings, SelectionState, HierarchyState, VisualizationMode};
use crate::gizmo::{GizmoMode, GizmoState, SnapSettings};
use crate::node_system::{NodeRegistry, NodeCategory};
use crate::plugin_core::PluginHost;
use crate::ui_api::UiEvent;

// Phosphor icons for toolbar
use egui_phosphor::regular::{
    ARROWS_OUT_CARDINAL, ARROW_CLOCKWISE, ARROWS_OUT, PLAY, PAUSE, STOP, GEAR,
    CUBE, LIGHTBULB, VIDEO_CAMERA, PLUS, CARET_DOWN, EYE, IMAGE, POLYGON,
    SUN, CLOUD, MAGNET,
};

pub fn render_toolbar(
    ctx: &egui::Context,
    gizmo: &mut GizmoState,
    settings: &mut EditorSettings,
    _menu_bar_height: f32,
    toolbar_height: f32,
    _window_width: f32,
    plugin_host: &PluginHost,
    registry: &NodeRegistry,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
) -> Vec<UiEvent> {
    let mut events = Vec::new();
    let api = plugin_host.api();

    egui::TopBottomPanel::top("toolbar")
        .exact_height(toolbar_height)
        .show(ctx, |ui| {
            let available_width = ui.available_width();

            ui.horizontal_centered(|ui| {
                // Calculate approximate width of all toolbar content
                // 3 transform + sep + 4 dropdowns + sep + 4 toggles + 1 dropdown + sep + 3 play + settings
                let button_size = Vec2::new(28.0, 24.0);
                let dropdown_size = 38.0;
                let sep_size = 16.0;
                let total_width = (3.0 * button_size.x) + sep_size + (4.0 * dropdown_size) + sep_size
                    + (4.0 * button_size.x) + dropdown_size + sep_size + (3.0 * button_size.x) + button_size.x + 40.0;

                // Center the content
                let padding = ((available_width - total_width) / 2.0).max(8.0);
                ui.add_space(padding);
                let active_color = Color32::from_rgb(66, 150, 250);
                let inactive_color = Color32::from_rgb(46, 46, 56);

                // === Transform Tools ===
                let is_translate = gizmo.mode == GizmoMode::Translate;
                let translate_resp = tool_button(ui, ARROWS_OUT_CARDINAL, button_size, is_translate, active_color, inactive_color);
                if translate_resp.clicked() {
                    gizmo.mode = GizmoMode::Translate;
                }
                translate_resp.on_hover_text("Move (W)");

                let is_rotate = gizmo.mode == GizmoMode::Rotate;
                let rotate_resp = tool_button(ui, ARROW_CLOCKWISE, button_size, is_rotate, active_color, inactive_color);
                if rotate_resp.clicked() {
                    gizmo.mode = GizmoMode::Rotate;
                }
                rotate_resp.on_hover_text("Rotate (E)");

                let is_scale = gizmo.mode == GizmoMode::Scale;
                let scale_resp = tool_button(ui, ARROWS_OUT, button_size, is_scale, active_color, inactive_color);
                if scale_resp.clicked() {
                    gizmo.mode = GizmoMode::Scale;
                }
                scale_resp.on_hover_text("Scale (R)");

                ui.add_space(4.0);

                // === Snap Dropdown ===
                let any_snap_enabled = gizmo.snap.translate_enabled || gizmo.snap.rotate_enabled || gizmo.snap.scale_enabled;
                let snap_color = if any_snap_enabled {
                    Color32::from_rgb(140, 191, 242)
                } else {
                    Color32::from_rgb(140, 140, 150)
                };
                snap_dropdown(ui, MAGNET, "Snap", snap_color, inactive_color, &mut gizmo.snap);

                separator(ui);

                // === Add Object Dropdowns ===
                let mesh_color = Color32::from_rgb(242, 166, 115);
                let light_color = Color32::from_rgb(255, 230, 140);
                let camera_color = Color32::from_rgb(140, 191, 242);

                // Meshes dropdown
                dropdown_button(ui, CUBE, "Mesh", mesh_color, inactive_color, |ui| {
                    for def in registry.get_by_category(NodeCategory::Meshes) {
                        if menu_item(ui, def.display_name) {
                            let entity = (def.spawn_fn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }
                });

                // Lights dropdown
                dropdown_button(ui, LIGHTBULB, "Light", light_color, inactive_color, |ui| {
                    for def in registry.get_by_category(NodeCategory::Lights) {
                        if menu_item(ui, def.display_name) {
                            let entity = (def.spawn_fn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }
                });

                // Camera dropdown
                dropdown_button(ui, VIDEO_CAMERA, "Camera", camera_color, inactive_color, |ui| {
                    for def in registry.get_by_category(NodeCategory::Cameras) {
                        if menu_item(ui, def.display_name) {
                            let entity = (def.spawn_fn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }
                });

                // More objects dropdown
                let more_color = Color32::from_rgb(160, 160, 175);
                dropdown_button(ui, PLUS, "More", more_color, inactive_color, |ui| {
                    // 3D Nodes
                    ui.label(RichText::new("Nodes").small().color(Color32::from_rgb(120, 120, 130)));
                    for def in registry.get_by_category(NodeCategory::Nodes3D) {
                        if menu_item(ui, def.display_name) {
                            let entity = (def.spawn_fn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }

                    ui.separator();

                    // Physics
                    ui.label(RichText::new("Physics").small().color(Color32::from_rgb(120, 120, 130)));
                    for def in registry.get_by_category(NodeCategory::Physics) {
                        if menu_item(ui, def.display_name) {
                            let entity = (def.spawn_fn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }

                    ui.separator();

                    // Environment
                    ui.label(RichText::new("Environment").small().color(Color32::from_rgb(120, 120, 130)));
                    for def in registry.get_by_category(NodeCategory::Environment) {
                        if menu_item(ui, def.display_name) {
                            let entity = (def.spawn_fn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }
                });

                separator(ui);

                // === Render Toggles ===
                let toggle_on_color = Color32::from_rgb(66, 150, 250);
                let toggle_off_color = Color32::from_rgb(60, 60, 70);

                // Textures toggle
                let tex_resp = tool_button(
                    ui, IMAGE, button_size,
                    settings.render_toggles.textures,
                    toggle_on_color, toggle_off_color
                );
                if tex_resp.clicked() {
                    settings.render_toggles.textures = !settings.render_toggles.textures;
                }
                tex_resp.on_hover_text(if settings.render_toggles.textures { "Textures: ON" } else { "Textures: OFF" });

                // Wireframe toggle
                let wire_resp = tool_button(
                    ui, POLYGON, button_size,
                    settings.render_toggles.wireframe,
                    toggle_on_color, toggle_off_color
                );
                if wire_resp.clicked() {
                    settings.render_toggles.wireframe = !settings.render_toggles.wireframe;
                }
                wire_resp.on_hover_text(if settings.render_toggles.wireframe { "Wireframe: ON" } else { "Wireframe: OFF" });

                // Lighting toggle
                let light_resp = tool_button(
                    ui, SUN, button_size,
                    settings.render_toggles.lighting,
                    toggle_on_color, toggle_off_color
                );
                if light_resp.clicked() {
                    settings.render_toggles.lighting = !settings.render_toggles.lighting;
                }
                light_resp.on_hover_text(if settings.render_toggles.lighting { "Lighting: ON" } else { "Lighting: OFF" });

                // Shadows toggle
                let shadow_resp = tool_button(
                    ui, CLOUD, button_size,
                    settings.render_toggles.shadows,
                    toggle_on_color, toggle_off_color
                );
                if shadow_resp.clicked() {
                    settings.render_toggles.shadows = !settings.render_toggles.shadows;
                }
                shadow_resp.on_hover_text(if settings.render_toggles.shadows { "Shadows: ON" } else { "Shadows: OFF" });

                ui.add_space(4.0);

                // === Visualization Mode Dropdown ===
                let viz_color = Color32::from_rgb(180, 180, 200);
                dropdown_button(ui, EYE, "Visualization", viz_color, inactive_color, |ui| {
                    for mode in VisualizationMode::ALL {
                        let is_selected = settings.visualization_mode == *mode;
                        let label = if is_selected {
                            format!("• {}", mode.label())
                        } else {
                            format!("  {}", mode.label())
                        };
                        if menu_item(ui, &label) {
                            settings.visualization_mode = *mode;
                            ui.close();
                        }
                    }
                });

                separator(ui);

                // === Play Controls ===
                let play_color = Color32::from_rgb(64, 166, 89);
                let play_resp = tool_button(ui, PLAY, button_size, false, play_color, inactive_color);
                play_resp.on_hover_text("Play");

                let pause_resp = tool_button(ui, PAUSE, button_size, false, active_color, inactive_color);
                pause_resp.on_hover_text("Pause");

                let stop_resp = tool_button(ui, STOP, button_size, false, Color32::from_rgb(200, 80, 80), inactive_color);
                stop_resp.on_hover_text("Stop");

                // === Plugin Toolbar Items ===
                if !api.toolbar_items.is_empty() {
                    separator(ui);

                    for (item, _plugin_id) in &api.toolbar_items {
                        let resp = tool_button(ui, &item.icon, button_size, false, active_color, inactive_color);
                        if resp.clicked() {
                            events.push(UiEvent::ButtonClicked(crate::ui_api::UiId(item.id.0)));
                        }
                        resp.on_hover_text(&item.tooltip);
                    }
                }

                separator(ui);

                // === Settings ===
                let settings_resp = tool_button(ui, GEAR, button_size, settings.show_settings_window, active_color, inactive_color);
                if settings_resp.clicked() {
                    settings.show_settings_window = !settings.show_settings_window;
                }
                settings_resp.on_hover_text("Settings");
            });
        });

    // Keep hierarchy reference alive
    let _ = hierarchy;

    events
}

fn separator(ui: &mut egui::Ui) {
    ui.add_space(8.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [Pos2::new(rect.left(), rect.top() + 6.0), Pos2::new(rect.left(), rect.bottom() - 6.0)],
        egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 70)),
    );
    ui.add_space(8.0);
}

fn tool_button(
    ui: &mut egui::Ui,
    icon: &str,
    size: Vec2,
    active: bool,
    active_color: Color32,
    inactive_color: Color32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let bg_color = if active {
            active_color
        } else if response.hovered() {
            Color32::from_rgb(56, 56, 68)
        } else {
            inactive_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(4), bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );
    }

    response
}

fn dropdown_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    icon_color: Color32,
    bg_color: Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let button_id = ui.make_persistent_id(label);
    let size = Vec2::new(38.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            Color32::from_rgb(56, 56, 68)
        } else {
            bg_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);

        // Icon
        ui.painter().text(
            Pos2::new(rect.left() + 12.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(13.0),
            icon_color,
        );

        // Caret
        ui.painter().text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            CARET_DOWN,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(140, 140, 150),
        );
    }

    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(button_id));
    }

    egui::popup_below_widget(
        ui,
        button_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(120.0);
            ui.style_mut().spacing.item_spacing.y = 2.0;
            add_contents(ui);
        },
    );

    response.on_hover_text(label);
}

fn menu_item(ui: &mut egui::Ui, label: &str) -> bool {
    let response = ui.add(
        egui::Button::new(label)
            .fill(Color32::TRANSPARENT)
            .corner_radius(CornerRadius::same(2))
            .min_size(Vec2::new(ui.available_width(), 0.0))
    );
    response.clicked()
}

fn snap_dropdown(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    icon_color: Color32,
    bg_color: Color32,
    snap: &mut SnapSettings,
) {
    let button_id = ui.make_persistent_id("snap_dropdown");
    let size = Vec2::new(38.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            Color32::from_rgb(56, 56, 68)
        } else {
            bg_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);

        // Icon
        ui.painter().text(
            Pos2::new(rect.left() + 12.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(13.0),
            icon_color,
        );

        // Caret
        ui.painter().text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            CARET_DOWN,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(140, 140, 150),
        );
    }

    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(button_id));
    }

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

    response.on_hover_text(label);
}
