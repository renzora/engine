//! Material Inspector Panel — shows properties for the selected node
//! and editable pin values (constants, colors, sliders).

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText, Slider};
use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;
use renzora_material::graph::*;
use renzora_material::nodes;

use crate::MaterialEditorState;

pub struct MaterialInspectorPanel;

impl EditorPanel for MaterialInspectorPanel {
    fn id(&self) -> &str {
        "material_inspector"
    }

    fn title(&self) -> &str {
        "Material Properties"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::SLIDERS_HORIZONTAL)
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
        let Some(editor_state) = world.get_resource::<MaterialEditorState>() else {
            return;
        };

        let text_muted = theme.text.muted.to_color32();
        let text_color = theme.text.primary.to_color32();

        // ── Material name and domain ──
        ui.horizontal(|ui| {
            ui.label(RichText::new("Name:").size(11.0).color(text_muted));
            let mut name = editor_state.graph.name.clone();
            if ui.text_edit_singleline(&mut name).changed() {
                cmds.push(move |world: &mut World| {
                    world.resource_mut::<MaterialEditorState>().graph.name = name;
                });
            }
        });

        ui.horizontal(|ui| {
            ui.label(RichText::new("Domain:").size(11.0).color(text_muted));
            ui.label(RichText::new(editor_state.graph.domain.display_name()).size(11.0).color(text_color));
        });

        ui.separator();

        // ── Selected node properties ──
        let Some(selected_id) = editor_state.selected_node else {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(RichText::new("Select a node to edit its properties").size(11.0).color(text_muted));
            });
            return;
        };

        let Some(node) = editor_state.graph.get_node(selected_id) else {
            return;
        };

        let def = nodes::node_def(&node.node_type);
        let display_name = def.map(|d| d.display_name).unwrap_or("Unknown");
        let description = def.map(|d| d.description).unwrap_or("");

        ui.label(RichText::new(display_name).size(14.0).color(text_color).strong());
        if !description.is_empty() {
            ui.label(RichText::new(description).size(10.0).color(text_muted));
        }

        ui.add_space(8.0);

        // ── Editable input pin values ──
        let pins = def.map(|d| (d.pins)()).unwrap_or_default();
        let input_pins: Vec<_> = pins
            .iter()
            .filter(|p| p.direction == PinDir::Input)
            .collect();

        if input_pins.is_empty() {
            ui.label(RichText::new("No editable properties").size(11.0).color(text_muted));
            return;
        }

        let mut graph_clone = editor_state.graph.clone();
        let mut any_changed = false;

        // Check which inputs are connected (connected inputs shouldn't be editable)
        let connected_pins: Vec<String> = editor_state.graph.connections
            .iter()
            .filter(|c| c.to_node == selected_id)
            .map(|c| c.to_pin.clone())
            .collect();

        for pin in &input_pins {
            let is_connected = connected_pins.contains(&pin.name);

            ui.horizontal(|ui| {
                ui.label(RichText::new(&pin.label).size(11.0).color(text_muted));
                if is_connected {
                    ui.label(RichText::new("(connected)").size(10.0).color(
                        egui::Color32::from_rgb(100, 150, 255),
                    ));
                }
            });

            if is_connected {
                continue;
            }

            let node_mut = graph_clone.get_node_mut(selected_id).unwrap();
            let current = node_mut
                .input_values
                .get(&pin.name)
                .cloned()
                .unwrap_or(pin.default_value.clone());

            match pin.pin_type {
                PinType::Float => {
                    let mut val = match &current {
                        PinValue::Float(f) => *f,
                        _ => 0.0,
                    };
                    let slider = Slider::new(&mut val, 0.0..=1.0).text("").step_by(0.01);
                    if ui.add(slider).changed() {
                        node_mut.input_values.insert(pin.name.clone(), PinValue::Float(val));
                        any_changed = true;
                    }
                }
                PinType::Vec2 => {
                    let mut v = match &current {
                        PinValue::Vec2(a) => *a,
                        _ => [0.0, 0.0],
                    };
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("X").size(10.0).color(text_muted));
                        changed |= ui.add(egui::DragValue::new(&mut v[0]).speed(0.01).range(-100.0..=100.0)).changed();
                        ui.label(RichText::new("Y").size(10.0).color(text_muted));
                        changed |= ui.add(egui::DragValue::new(&mut v[1]).speed(0.01).range(-100.0..=100.0)).changed();
                    });
                    if changed {
                        node_mut.input_values.insert(pin.name.clone(), PinValue::Vec2(v));
                        any_changed = true;
                    }
                }
                PinType::Vec3 => {
                    let mut v = match &current {
                        PinValue::Vec3(a) => *a,
                        _ => [0.0, 0.0, 0.0],
                    };
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("X").size(10.0).color(text_muted));
                        changed |= ui.add(egui::DragValue::new(&mut v[0]).speed(0.01).range(-100.0..=100.0)).changed();
                        ui.label(RichText::new("Y").size(10.0).color(text_muted));
                        changed |= ui.add(egui::DragValue::new(&mut v[1]).speed(0.01).range(-100.0..=100.0)).changed();
                        ui.label(RichText::new("Z").size(10.0).color(text_muted));
                        changed |= ui.add(egui::DragValue::new(&mut v[2]).speed(0.01).range(-100.0..=100.0)).changed();
                    });
                    if changed {
                        node_mut.input_values.insert(pin.name.clone(), PinValue::Vec3(v));
                        any_changed = true;
                    }
                }
                PinType::Color => {
                    let mut c = match &current {
                        PinValue::Color(a) => *a,
                        _ => [1.0, 1.0, 1.0, 1.0],
                    };
                    let mut color3 = [c[0], c[1], c[2]];
                    if ui.color_edit_button_rgb(&mut color3).changed() {
                        c[0] = color3[0]; c[1] = color3[1]; c[2] = color3[2];
                        node_mut.input_values.insert(pin.name.clone(), PinValue::Color(c));
                        any_changed = true;
                    }
                }
                PinType::Bool => {
                    let mut val = match &current {
                        PinValue::Bool(b) => *b,
                        _ => false,
                    };
                    if ui.checkbox(&mut val, "").changed() {
                        node_mut.input_values.insert(pin.name.clone(), PinValue::Bool(val));
                        any_changed = true;
                    }
                }
                PinType::Texture2D => {
                    let path = match &current {
                        PinValue::TexturePath(s) => s.clone(),
                        _ => String::new(),
                    };
                    ui.horizontal(|ui| {
                        let mut p = path.clone();
                        if ui.text_edit_singleline(&mut p).changed() {
                            node_mut.input_values.insert(pin.name.clone(), PinValue::TexturePath(p));
                            any_changed = true;
                        }
                        if ui.small_button(egui_phosphor::regular::FOLDER_OPEN).clicked() {
                            #[cfg(not(target_arch = "wasm32"))]
                            if let Some(file) = rfd::FileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg", "ktx2"])
                                .pick_file()
                            {
                                let new_path = if let Some(project) = world.get_resource::<renzora_core::CurrentProject>() {
                                    project.make_asset_relative(&file)
                                } else {
                                    file.to_string_lossy().to_string()
                                };
                                node_mut.input_values.insert(pin.name.clone(), PinValue::TexturePath(new_path));
                                any_changed = true;
                            }
                        }
                    });
                }
                _ => {
                    ui.label(RichText::new("(no editor)").size(10.0).color(text_muted));
                }
            }

            ui.add_space(2.0);
        }

        if any_changed {
            cmds.push(move |world: &mut World| {
                let mut state = world.resource_mut::<MaterialEditorState>();
                state.graph = graph_clone;
                state.is_modified = true;
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
