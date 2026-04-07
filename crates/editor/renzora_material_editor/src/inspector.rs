//! Material Inspector Panel — shows properties for the selected node
//! and editable pin values (constants, colors, sliders).

use bevy::prelude::*;
use renzora::bevy_egui::egui::{self, RichText, Slider};
use renzora::editor::{
    collapsible_section, inline_property, empty_state,
    EditorCommands, EditorPanel, PanelLocation,
};
use renzora::theme::ThemeManager;
use renzora_shader::material::graph::*;
use renzora_shader::material::nodes;
use renzora_ui::asset_drag::{asset_drop_target, AssetDragPayload};

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
        Some(renzora::egui_phosphor::regular::SLIDERS_HORIZONTAL)
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

        // ── Material section ──
        collapsible_section(
            ui,
            renzora::egui_phosphor::regular::CUBE,
            "Material",
            "rendering",
            &theme,
            "mat_insp_material",
            true,
            |ui| {
                let mut row = 0;
                inline_property(ui, row, "Name", &theme, |ui| {
                    let mut name = editor_state.graph.name.clone();
                    let resp = ui.text_edit_singleline(&mut name).changed();
                    if resp {
                        cmds.push(move |world: &mut World| {
                            world.resource_mut::<MaterialEditorState>().graph.name = name;
                        });
                    }
                    resp
                });
                row += 1;
                inline_property(ui, row, "Domain", &theme, |ui| {
                    ui.label(
                        RichText::new(editor_state.graph.domain.display_name())
                            .size(11.0)
                            .color(theme.text.primary.to_color32()),
                    );
                    false
                });
            },
        );

        // ── Selected node properties ──
        let Some(selected_id) = editor_state.selected_node else {
            empty_state(ui, renzora::egui_phosphor::regular::CURSOR_CLICK, "No node selected", "Select a node to edit its properties", &theme);
            return;
        };

        let Some(node) = editor_state.graph.get_node(selected_id) else {
            return;
        };

        let def = nodes::node_def(&node.node_type);
        let display_name = def.map(|d| d.display_name).unwrap_or("Unknown");
        let category = def.map(|d| d.category).unwrap_or("Utility");
        let description = def.map(|d| d.description).unwrap_or("");
        let icon = crate::graph_editor::category_icon(category);
        let theme_cat = match category {
            "Texture" | "Procedural" => "rendering",
            "Math" | "Vector" | "Utility" => "scripting",
            "Color" => "effects",
            "Animation" => "effects",
            "Input" => "transform",
            "Output" => "rendering",
            _ => "rendering",
        };

        // ── Editable input pin values ──
        let pins = def.map(|d| (d.pins)()).unwrap_or_default();
        let input_pins: Vec<_> = pins
            .iter()
            .filter(|p| p.direction == PinDir::Input)
            .collect();

        let mut graph_clone = editor_state.graph.clone();
        let mut any_changed = false;

        // Check which inputs are connected
        let connected_pins: Vec<String> = editor_state.graph.connections
            .iter()
            .filter(|c| c.to_node == selected_id)
            .map(|c| c.to_pin.clone())
            .collect();

        collapsible_section(
            ui,
            icon,
            display_name,
            theme_cat,
            &theme,
            &format!("mat_insp_node_{}", selected_id),
            true,
            |ui| {
                if !description.is_empty() {
                    ui.label(RichText::new(description).size(10.0).color(theme.text.muted.to_color32()));
                    ui.add_space(4.0);
                }

                if input_pins.is_empty() {
                    ui.label(RichText::new("No editable properties").size(11.0).color(theme.text.muted.to_color32()));
                    return;
                }

                for (row, pin) in input_pins.iter().enumerate() {
                    let is_connected = connected_pins.contains(&pin.name);

                    if is_connected {
                        inline_property(ui, row, &pin.label, &theme, |ui| {
                            ui.label(RichText::new("(connected)").size(10.0).color(
                                egui::Color32::from_rgb(100, 150, 255),
                            ));
                            false
                        });
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
                            inline_property(ui, row, &pin.label, &theme, |ui| {
                                let mut val = match &current {
                                    PinValue::Float(f) => *f,
                                    _ => 0.0,
                                };
                                if ui.add(egui::DragValue::new(&mut val).speed(0.01).range(-1000.0..=1000.0)).changed() {
                                    node_mut.input_values.insert(pin.name.clone(), PinValue::Float(val));
                                    any_changed = true;
                                    true
                                } else {
                                    false
                                }
                            });
                        }
                        PinType::Vec2 => {
                            inline_property(ui, row, &pin.label, &theme, |ui| {
                                let mut v = match &current {
                                    PinValue::Vec2(a) => *a,
                                    _ => [0.0, 0.0],
                                };
                                let mut changed = false;
                                let text_muted = theme.text.muted.to_color32();
                                ui.label(RichText::new("X").size(10.0).color(text_muted));
                                changed |= ui.add(egui::DragValue::new(&mut v[0]).speed(0.1).range(-10000.0..=10000.0)).changed();
                                ui.label(RichText::new("Y").size(10.0).color(text_muted));
                                changed |= ui.add(egui::DragValue::new(&mut v[1]).speed(0.1).range(-10000.0..=10000.0)).changed();
                                if changed {
                                    node_mut.input_values.insert(pin.name.clone(), PinValue::Vec2(v));
                                    any_changed = true;
                                }
                                changed
                            });
                        }
                        PinType::Vec3 => {
                            inline_property(ui, row, &pin.label, &theme, |ui| {
                                let mut v = match &current {
                                    PinValue::Vec3(a) => *a,
                                    _ => [0.0, 0.0, 0.0],
                                };
                                let mut changed = false;
                                let text_muted = theme.text.muted.to_color32();
                                ui.label(RichText::new("X").size(10.0).color(text_muted));
                                changed |= ui.add(egui::DragValue::new(&mut v[0]).speed(0.1).range(-10000.0..=10000.0)).changed();
                                ui.label(RichText::new("Y").size(10.0).color(text_muted));
                                changed |= ui.add(egui::DragValue::new(&mut v[1]).speed(0.1).range(-10000.0..=10000.0)).changed();
                                ui.label(RichText::new("Z").size(10.0).color(text_muted));
                                changed |= ui.add(egui::DragValue::new(&mut v[2]).speed(0.1).range(-10000.0..=10000.0)).changed();
                                if changed {
                                    node_mut.input_values.insert(pin.name.clone(), PinValue::Vec3(v));
                                    any_changed = true;
                                }
                                changed
                            });
                        }
                        PinType::Color => {
                            inline_property(ui, row, &pin.label, &theme, |ui| {
                                let mut c = match &current {
                                    PinValue::Color(a) => *a,
                                    _ => [1.0, 1.0, 1.0, 1.0],
                                };
                                let mut color3 = [c[0], c[1], c[2]];
                                if ui.color_edit_button_rgb(&mut color3).changed() {
                                    c[0] = color3[0]; c[1] = color3[1]; c[2] = color3[2];
                                    node_mut.input_values.insert(pin.name.clone(), PinValue::Color(c));
                                    any_changed = true;
                                    true
                                } else {
                                    false
                                }
                            });
                        }
                        PinType::Bool => {
                            inline_property(ui, row, &pin.label, &theme, |ui| {
                                let mut val = match &current {
                                    PinValue::Bool(b) => *b,
                                    _ => false,
                                };
                                if ui.checkbox(&mut val, "").changed() {
                                    node_mut.input_values.insert(pin.name.clone(), PinValue::Bool(val));
                                    any_changed = true;
                                    true
                                } else {
                                    false
                                }
                            });
                        }
                        PinType::Texture2D => {
                            let path = match &current {
                                PinValue::TexturePath(s) => s.clone(),
                                _ => String::new(),
                            };
                            inline_property(ui, row, &pin.label, &theme, |ui| {
                                let drag_payload = world.get_resource::<AssetDragPayload>();
                                let drop_result = asset_drop_target(
                                    ui,
                                    egui::Id::new(("mat_tex_drop", selected_id, &pin.name)),
                                    if path.is_empty() { None } else { Some(path.as_str()) },
                                    &["png", "jpg", "jpeg", "ktx2", "tga", "bmp", "dds", "exr", "hdr", "webp"],
                                    "Drop texture or click to browse",
                                    &theme,
                                    drag_payload,
                                );
                                if let Some(ref dropped) = drop_result.dropped_path {
                                    let new_path = if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
                                        project.make_asset_relative(&dropped)
                                    } else {
                                        dropped.to_string_lossy().to_string()
                                    };
                                    node_mut.input_values.insert(pin.name.clone(), PinValue::TexturePath(new_path));
                                    any_changed = true;
                                }
                                if drop_result.cleared {
                                    node_mut.input_values.insert(pin.name.clone(), PinValue::TexturePath(String::new()));
                                    any_changed = true;
                                }
                                if drop_result.browse_clicked {
                                    #[cfg(not(target_arch = "wasm32"))]
                                    if let Some(file) = rfd::FileDialog::new()
                                        .add_filter("Image", &["png", "jpg", "jpeg", "ktx2", "tga", "bmp", "dds", "exr", "hdr", "webp"])
                                        .pick_file()
                                    {
                                        let new_path = if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
                                            project.make_asset_relative(&file)
                                        } else {
                                            file.to_string_lossy().to_string()
                                        };
                                        node_mut.input_values.insert(pin.name.clone(), PinValue::TexturePath(new_path));
                                        any_changed = true;
                                    }
                                }
                                drop_result.dropped_path.is_some() || drop_result.cleared || drop_result.browse_clicked
                            });
                        }
                        _ => {
                            inline_property(ui, row, &pin.label, &theme, |ui| {
                                ui.label(RichText::new("(no editor)").size(10.0).color(theme.text.muted.to_color32()));
                                false
                            });
                        }
                    }
                }
            },
        );

        if any_changed {
            cmds.push(move |world: &mut World| {
                let mut state = world.resource_mut::<MaterialEditorState>();
                state.graph = graph_clone;
                state.is_dirty = true;
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
