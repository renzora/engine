//! Lifecycle Node Properties — inspector for the selected node's input values.

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular::{SLIDERS, FLOW_ARROW, PLUGS_CONNECTED, PLUG};

use renzora_blueprint::graph::*;
use renzora_lifecycle::graph::LifecycleGraph;
use renzora_lifecycle::nodes;
use renzora_editor_framework::{collapsible_section, EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;
use renzora_ui::{AssetDragPayload, asset_drop_target};

use crate::LifecycleEditorState;

pub struct LifecyclePropertiesPanel;

impl EditorPanel for LifecyclePropertiesPanel {
    fn id(&self) -> &str {
        "lifecycle_properties"
    }

    fn title(&self) -> &str {
        "Node Properties"
    }

    fn icon(&self) -> Option<&str> {
        Some(SLIDERS)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
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
        let Some(editor_state) = world.get_resource::<LifecycleEditorState>() else {
            return;
        };
        let Some(graph) = world.get_resource::<LifecycleGraph>() else {
            return;
        };

        let text_muted = theme.text.muted.to_color32();
        let text_primary = theme.text.primary.to_color32();
        let text_secondary = theme.text.secondary.to_color32();

        let Some(selected_id) = editor_state.selected_node else {
            centered_message(ui, "Select a node to edit its properties", text_muted);
            return;
        };

        let Some(node) = graph.get_node(selected_id) else {
            centered_message(ui, "Node not found", text_muted);
            return;
        };

        let Some(def) = nodes::node_def(&node.node_type) else {
            centered_message(ui, "Unknown node type", text_muted);
            return;
        };

        let pins = (def.pins)();
        let _header_color = egui::Color32::from_rgb(def.color[0], def.color[1], def.color[2]);

        // Gather input pins (non-exec)
        let input_pins: Vec<&PinTemplate> = pins
            .iter()
            .filter(|p| p.direction == PinDir::Input && p.pin_type != PinType::Exec)
            .collect();

        // Gather output pins (non-exec)
        let output_pins: Vec<&PinTemplate> = pins
            .iter()
            .filter(|p| p.direction == PinDir::Output && p.pin_type != PinType::Exec)
            .collect();

        // Connections into this node
        let connections_to_node: Vec<&BlueprintConnection> = graph
            .connections
            .iter()
            .filter(|c| c.to_node == selected_id)
            .collect();

        let mut updated_values: Vec<(String, PinValue)> = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                // ── Node Info section ──
                collapsible_section(
                    ui,
                    FLOW_ARROW,
                    def.display_name,
                    "scripting",
                    &theme,
                    &format!("lc_props_info_{}", selected_id),
                    true,
                    |ui| {
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(def.description)
                                .size(11.0)
                                .color(text_secondary),
                        );
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Type")
                                    .size(10.0)
                                    .color(text_muted),
                            );
                            ui.label(
                                RichText::new(&node.node_type)
                                    .size(10.0)
                                    .color(text_muted),
                            );
                        });
                    },
                );

                // ── Inputs section ──
                if !input_pins.is_empty() {
                    collapsible_section(
                        ui,
                        PLUG,
                        "Inputs",
                        "transform",
                        &theme,
                        &format!("lc_props_inputs_{}", selected_id),
                        true,
                        |ui| {
                            for pin in &input_pins {
                                let is_connected = connections_to_node
                                    .iter()
                                    .any(|c| c.to_pin == pin.name);

                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
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
                                    ui.label(
                                        RichText::new(pin_type_label(pin.pin_type))
                                            .size(10.0)
                                            .color(text_muted),
                                    );
                                });

                                if is_connected {
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
                                    let current = node
                                        .get_input_value(&pin.name)
                                        .cloned()
                                        .unwrap_or_else(|| pin.default_value.clone());

                                    if let Some(new_val) = render_pin_editor(
                                        ui, &pin.name, pin.pin_type, &current, &theme, world,
                                    ) {
                                        updated_values.push((pin.name.clone(), new_val));
                                    }
                                }

                                ui.add_space(2.0);
                            }
                        },
                    );
                }

                // ── Outputs section ──
                if !output_pins.is_empty() {
                    collapsible_section(
                        ui,
                        PLUGS_CONNECTED,
                        "Outputs",
                        "environment",
                        &theme,
                        &format!("lc_props_outputs_{}", selected_id),
                        true,
                        |ui| {
                            for pin in &output_pins {
                                ui.horizontal(|ui| {
                                    ui.add_space(4.0);
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
                        },
                    );
                }
            });

        // Apply changes
        if !updated_values.is_empty() {
            let node_id = selected_id;
            cmds.push(move |world: &mut World| {
                let mut graph = world.resource_mut::<LifecycleGraph>();
                if let Some(node) = graph.get_node_mut(node_id) {
                    for (name, value) in updated_values {
                        node.input_values.insert(name, value);
                    }
                }
            });
        }
    }

    fn closable(&self) -> bool {
        true
    }
}

/// Render an editor widget for a pin value. Returns Some(new_value) if changed.
fn render_pin_editor(
    ui: &mut egui::Ui,
    pin_name: &str,
    pin_type: PinType,
    current: &PinValue,
    theme: &renzora_theme::Theme,
    world: &World,
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
            PinType::String if pin_name == "path" => {
                // Scene path — use asset drop target for .ron files
                let val = current.as_string();
                let current_str = if val.is_empty() { None } else { Some(val.as_str()) };
                let payload = world.get_resource::<AssetDragPayload>();
                let drop_result = asset_drop_target(
                    ui,
                    ui.id().with(pin_name),
                    current_str,
                    &["ron"],
                    "Drop scene here",
                    theme,
                    payload,
                );
                if let Some(path) = drop_result.dropped_path {
                    let path_str = if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
                        project.make_asset_relative(&path)
                    } else {
                        path.to_string_lossy().to_string()
                    };
                    changed = Some(PinValue::String(path_str));
                }
                if drop_result.cleared {
                    changed = Some(PinValue::String(String::new()));
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
                if ui
                    .color_edit_button_rgba_unmultiplied(&mut color)
                    .changed()
                {
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
                // For Any type, render as a string editor
                let mut val = current.as_string();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut val)
                        .desired_width(ui.available_width() - 32.0)
                        .hint_text("value..."),
                );
                if resp.changed() {
                    changed = Some(PinValue::String(val));
                }
            }
            PinType::Exec => {}
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
