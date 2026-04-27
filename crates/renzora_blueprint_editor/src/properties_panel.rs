//! Blueprint Node Properties — side panel for editing selected node's input values.

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular::{SLIDERS, FLOW_ARROW, PLUGS_CONNECTED, PLUG};

use renzora_blueprint::graph::*;
use renzora_blueprint::{BlueprintGraph, nodes};
use renzora_editor::{
    DocTabKind, EditorCommands, EditorContext, EditorPanel, PanelLocation,
};
use renzora_theme::ThemeManager;
use renzora::core::CurrentProject;

use crate::BlueprintEditorState;

#[derive(Default)]
pub struct BlueprintPropertiesPanel;

impl EditorPanel for BlueprintPropertiesPanel {
    fn id(&self) -> &str {
        "blueprint_properties"
    }

    fn title(&self) -> &str {
        "Node Properties"
    }

    fn icon(&self) -> Option<&str> {
        Some(SLIDERS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let cmds = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };
        let Some(bp_state) = world.get_resource::<BlueprintEditorState>() else {
            return;
        };

        let text_muted = theme.text.muted.to_color32();
        let text_secondary = theme.text.secondary.to_color32();
        let text_primary = theme.text.primary.to_color32();

        // Asset mode: graph comes from BlueprintEditorState::file_graph and
        // edits are persisted by saving the file. Scene mode: graph is the
        // BlueprintGraph component on the editing entity.
        let asset_mode = matches!(
            world.get_resource::<EditorContext>(),
            Some(EditorContext::Asset { kind: DocTabKind::Blueprint, .. })
        );

        let Some(selected_id) = bp_state.selected_node else {
            centered_message(ui, "Select a node to edit its properties", text_muted);
            return;
        };

        let (graph, scene_entity): (BlueprintGraph, Option<Entity>) = if asset_mode {
            let Some(g) = bp_state.file_graph.as_ref() else {
                centered_message(ui, "No blueprint loaded", text_muted);
                return;
            };
            (g.clone(), None)
        } else {
            let Some(entity) = bp_state.editing_entity else {
                centered_message(ui, "No entity selected", text_muted);
                return;
            };
            let Some(g) = world.get::<BlueprintGraph>(entity) else {
                centered_message(ui, "Entity has no blueprint", text_muted);
                return;
            };
            (g.clone(), Some(entity))
        };
        let graph = &graph;

        let Some(node) = graph.get_node(selected_id) else {
            centered_message(ui, "Node not found", text_muted);
            return;
        };

        let Some(def) = nodes::node_def(&node.node_type) else {
            centered_message(ui, "Unknown node type", text_muted);
            return;
        };

        let pins = (def.pins)();
        let header_color = egui::Color32::from_rgb(def.color[0], def.color[1], def.color[2]);

        // ── Header ──
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{FLOW_ARROW} {}", def.display_name))
                    .size(14.0)
                    .color(header_color)
                    .strong(),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(def.description)
                    .size(11.0)
                    .color(text_secondary),
            );
        });
        ui.add_space(4.0);
        ui.separator();

        // ── Connection status ──
        let connections_to_node: Vec<&BlueprintConnection> = graph
            .connections
            .iter()
            .filter(|c| c.to_node == selected_id)
            .collect();

        // ── Input pins ──
        let input_pins: Vec<&PinTemplate> = pins
            .iter()
            .filter(|p| p.direction == PinDir::Input && p.pin_type != PinType::Exec)
            .collect();

        if input_pins.is_empty() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("No editable inputs")
                        .size(12.0)
                        .color(text_muted),
                );
            });
            return;
        }

        // Clone values we need for mutation
        let mut updated_values: Vec<(String, PinValue)> = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for pin in &input_pins {
                    let is_connected = connections_to_node
                        .iter()
                        .any(|c| c.to_pin == pin.name);

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);

                        // Connection indicator
                        let conn_icon = if is_connected { PLUGS_CONNECTED } else { PLUG };
                        let conn_color = if is_connected {
                            theme.semantic.accent.to_color32()
                        } else {
                            text_muted
                        };
                        ui.label(RichText::new(conn_icon).size(11.0).color(conn_color));

                        ui.label(
                            RichText::new(&pin.label)
                                .size(12.0)
                                .color(text_primary),
                        );

                        // Show type badge
                        ui.label(
                            RichText::new(pin_type_label(pin.pin_type))
                                .size(10.0)
                                .color(text_muted),
                        );
                    });

                    if is_connected {
                        // Pin is wired — show "connected" hint, value comes from wire
                        ui.horizontal(|ui| {
                            ui.add_space(24.0);
                            ui.label(
                                RichText::new("Value from connection")
                                    .size(11.0)
                                    .italics()
                                    .color(text_muted),
                            );
                        });
                    } else {
                        // Editable field
                        let current = node
                            .get_input_value(&pin.name)
                            .cloned()
                            .unwrap_or_else(|| pin.default_value.clone());

                        if let Some(new_val) = render_pin_editor(ui, &pin.name, pin.pin_type, &current, &theme) {
                            updated_values.push((pin.name.clone(), new_val));
                        }
                    }

                    ui.add_space(2.0);
                }

                // ── Output pins (read-only info) ──
                let output_pins: Vec<&PinTemplate> = pins
                    .iter()
                    .filter(|p| p.direction == PinDir::Output && p.pin_type != PinType::Exec)
                    .collect();

                if !output_pins.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Outputs")
                                .size(12.0)
                                .color(text_secondary)
                                .strong(),
                        );
                    });
                    for pin in &output_pins {
                        ui.horizontal(|ui| {
                            ui.add_space(24.0);
                            ui.label(
                                RichText::new(&pin.label)
                                    .size(11.0)
                                    .color(text_primary),
                            );
                            ui.label(
                                RichText::new(pin_type_label(pin.pin_type))
                                    .size(10.0)
                                    .color(text_muted),
                            );
                        });
                    }
                }
            });

        // Apply changes — write back to the entity component (scene mode)
        // or to BlueprintEditorState::file_graph (asset mode). The graph
        // panel's save loop will pick up the dirty flag and persist.
        if !updated_values.is_empty() {
            let node_id = selected_id;
            cmds.push(move |world: &mut World| {
                if let Some(entity) = scene_entity {
                    if let Some(mut graph) = world.get_mut::<BlueprintGraph>(entity) {
                        if let Some(node) = graph.get_node_mut(node_id) {
                            for (name, value) in updated_values {
                                node.input_values.insert(name, value);
                            }
                        }
                    }
                } else {
                    // Asset mode: mutate the in-memory graph and persist to
                    // the .blueprint file in one go — the graph panel only
                    // saves on its own edits, so we own the persistence path.
                    let (saved_path, saved_graph) = {
                        let mut state = world.resource_mut::<BlueprintEditorState>();
                        let path = state.editing_file_path.clone();
                        if let Some(graph) = state.file_graph.as_mut() {
                            if let Some(node) = graph.get_node_mut(node_id) {
                                for (name, value) in updated_values {
                                    node.input_values.insert(name, value);
                                }
                            }
                        }
                        let graph_clone = state.file_graph.clone();
                        state.is_dirty = true;
                        (path, graph_clone)
                    };
                    if let (Some(path), Some(graph)) = (saved_path, saved_graph) {
                        let project = world.get_resource::<CurrentProject>().cloned();
                        crate::graph_panel::save_blueprint_file(project.as_ref(), &path, &graph);
                        world.resource_mut::<BlueprintEditorState>().is_dirty = false;
                    }
                }
            });
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}

/// Render an editor widget for a pin value. Returns Some(new_value) if changed.
fn render_pin_editor(
    ui: &mut egui::Ui,
    _pin_name: &str,
    pin_type: PinType,
    current: &PinValue,
    theme: &renzora_theme::Theme,
) -> Option<PinValue> {
    let mut changed = None;

    ui.horizontal(|ui| {
        ui.add_space(24.0);

        match pin_type {
            PinType::Float => {
                let mut val = current.as_float();
                let resp = ui.add(
                    egui::DragValue::new(&mut val)
                        .speed(0.01)
                        .prefix("")
                        .range(f32::MIN..=f32::MAX),
                );
                if resp.changed() {
                    changed = Some(PinValue::Float(val));
                }
            }
            PinType::Int => {
                let mut val = current.as_int();
                let resp = ui.add(
                    egui::DragValue::new(&mut val)
                        .speed(0.1)
                        .range(i32::MIN..=i32::MAX),
                );
                if resp.changed() {
                    changed = Some(PinValue::Int(val));
                }
            }
            PinType::Bool => {
                let mut val = current.as_bool();
                if ui.checkbox(&mut val, "").changed() {
                    changed = Some(PinValue::Bool(val));
                }
            }
            PinType::String => {
                let mut val = current.as_string();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut val)
                        .desired_width(ui.available_width() - 32.0)
                        .hint_text("..."),
                );
                if resp.changed() {
                    changed = Some(PinValue::String(val));
                }
            }
            PinType::Vec2 => {
                let v = current.as_vec2();
                let mut x = v[0];
                let mut y = v[1];
                let mut any_changed = false;

                ui.label(RichText::new("X").size(10.0).color(theme.text.muted.to_color32()));
                if ui.add(egui::DragValue::new(&mut x).speed(0.01)).changed() {
                    any_changed = true;
                }
                ui.label(RichText::new("Y").size(10.0).color(theme.text.muted.to_color32()));
                if ui.add(egui::DragValue::new(&mut y).speed(0.01)).changed() {
                    any_changed = true;
                }

                if any_changed {
                    changed = Some(PinValue::Vec2([x, y]));
                }
            }
            PinType::Vec3 => {
                let v = current.as_vec3();
                let mut x = v[0];
                let mut y = v[1];
                let mut z = v[2];
                let mut any_changed = false;

                ui.label(RichText::new("X").size(10.0).color(theme.text.muted.to_color32()));
                if ui.add(egui::DragValue::new(&mut x).speed(0.01)).changed() {
                    any_changed = true;
                }
                ui.label(RichText::new("Y").size(10.0).color(theme.text.muted.to_color32()));
                if ui.add(egui::DragValue::new(&mut y).speed(0.01)).changed() {
                    any_changed = true;
                }
                ui.label(RichText::new("Z").size(10.0).color(theme.text.muted.to_color32()));
                if ui.add(egui::DragValue::new(&mut z).speed(0.01)).changed() {
                    any_changed = true;
                }

                if any_changed {
                    changed = Some(PinValue::Vec3([x, y, z]));
                }
            }
            PinType::Color => {
                let c = current.as_color();
                let mut color = [c[0], c[1], c[2], c[3]];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    changed = Some(PinValue::Color(color));
                }
            }
            PinType::Entity => {
                let mut val = current.as_string();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut val)
                        .desired_width(ui.available_width() - 32.0)
                        .hint_text("Entity name..."),
                );
                if resp.changed() {
                    changed = Some(PinValue::Entity(val));
                }
            }
            PinType::Any => {
                ui.label(
                    RichText::new("(any)")
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                );
            }
            PinType::Exec => {} // Not editable
        }
    });

    changed
}

fn pin_type_label(pin_type: PinType) -> &'static str {
    match pin_type {
        PinType::Exec => "exec",
        PinType::Float => "float",
        PinType::Int => "int",
        PinType::Bool => "bool",
        PinType::String => "string",
        PinType::Vec2 => "vec2",
        PinType::Vec3 => "vec3",
        PinType::Color => "color",
        PinType::Entity => "entity",
        PinType::Any => "any",
    }
}

fn centered_message(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.centered_and_justified(|ui| {
        ui.label(RichText::new(text).size(13.0).color(color));
    });
}
