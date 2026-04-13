//! Shader properties panel — edit exposed shader parameters using inspector-style sections.

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};

use renzora_editor_framework::{
    collapsible_section, inline_property,
    EditorCommands, EditorPanel, PanelLocation,
};
use renzora_shader::file::{ParamType, ParamValue};
use renzora_theme::ThemeManager;

use crate::ShaderEditorState;
use crate::code_panel::reapply_params;

pub struct ShaderPropertiesPanel;

impl EditorPanel for ShaderPropertiesPanel {
    fn id(&self) -> &str {
        "shader_properties"
    }

    fn title(&self) -> &str {
        "Shader Properties"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::SLIDERS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => &tm.active_theme,
            None => return,
        };
        let disabled = theme.text.disabled.to_color32();

        let Some(state) = world.get_resource::<ShaderEditorState>() else { return };

        if state.shader_file.params.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(RichText::new("No parameters defined").size(11.0).color(disabled));
                ui.label(
                    RichText::new("Add // @param annotations to your shader")
                        .size(10.0)
                        .color(disabled),
                );
            });
            return;
        }

        let mut params: Vec<(String, _)> = state
            .shader_file
            .params
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        params.sort_by(|a, b| a.0.cmp(&b.0));

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Group params by type for sectioning
            let float_params: Vec<_> = params.iter().filter(|(_, p)| p.param_type == ParamType::Float).collect();
            let color_params: Vec<_> = params.iter().filter(|(_, p)| p.param_type == ParamType::Color).collect();
            let vec_params: Vec<_> = params.iter().filter(|(_, p)| matches!(p.param_type, ParamType::Vec2 | ParamType::Vec3 | ParamType::Vec4)).collect();
            let int_params: Vec<_> = params.iter().filter(|(_, p)| p.param_type == ParamType::Int).collect();
            let bool_params: Vec<_> = params.iter().filter(|(_, p)| p.param_type == ParamType::Bool).collect();

            let mut row = 0usize;

            if !float_params.is_empty() {
                collapsible_section(
                    ui,
                    egui_phosphor::regular::SLIDERS_HORIZONTAL,
                    "Float",
                    "rendering",
                    theme,
                    "shader_props_float",
                    true,
                    |ui| {
                        for (name, param) in &float_params {
                            render_float_param(ui, world, name, param, theme, row);
                            row += 1;
                        }
                    },
                );
            }

            if !color_params.is_empty() {
                collapsible_section(
                    ui,
                    egui_phosphor::regular::PALETTE,
                    "Color",
                    "rendering",
                    theme,
                    "shader_props_color",
                    true,
                    |ui| {
                        for (name, param) in &color_params {
                            render_color_param(ui, world, name, param, theme, row);
                            row += 1;
                        }
                    },
                );
            }

            if !vec_params.is_empty() {
                collapsible_section(
                    ui,
                    egui_phosphor::regular::ARROWS_OUT_CARDINAL,
                    "Vector",
                    "rendering",
                    theme,
                    "shader_props_vec",
                    true,
                    |ui| {
                        for (name, param) in &vec_params {
                            render_vec_param(ui, world, name, param, theme, row);
                            row += 1;
                        }
                    },
                );
            }

            if !int_params.is_empty() {
                collapsible_section(
                    ui,
                    egui_phosphor::regular::HASH,
                    "Integer",
                    "rendering",
                    theme,
                    "shader_props_int",
                    true,
                    |ui| {
                        for (name, param) in &int_params {
                            render_int_param(ui, world, name, param, theme, row);
                            row += 1;
                        }
                    },
                );
            }

            if !bool_params.is_empty() {
                collapsible_section(
                    ui,
                    egui_phosphor::regular::TOGGLE_LEFT,
                    "Boolean",
                    "rendering",
                    theme,
                    "shader_props_bool",
                    true,
                    |ui| {
                        for (name, param) in &bool_params {
                            render_bool_param(ui, world, name, param, theme, row);
                            row += 1;
                        }
                    },
                );
            }
        });
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}

fn render_float_param(
    ui: &mut egui::Ui,
    world: &World,
    name: &str,
    param: &renzora_shader::file::ShaderParam,
    theme: &renzora_theme::Theme,
    row: usize,
) {
    let ParamValue::Float(v) = param.default_value else { return };
    let mut val = v;
    let min = param.min.unwrap_or(0.0);
    let max = param.max.unwrap_or(1.0);

    inline_property(ui, row, name, theme, |ui| {
        if ui.add(egui::Slider::new(&mut val, min..=max)).changed() {
            let n = name.to_string();
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                cmds.push(move |world: &mut World| {
                    let mut s = world.resource_mut::<ShaderEditorState>();
                    if let Some(p) = s.shader_file.params.get_mut(&n) {
                        p.default_value = ParamValue::Float(val);
                    }
                    s.is_modified = true;
                    drop(s);
                    reapply_params(world);
                });
            }
        }
    });
}

fn render_color_param(
    ui: &mut egui::Ui,
    world: &World,
    name: &str,
    param: &renzora_shader::file::ShaderParam,
    theme: &renzora_theme::Theme,
    row: usize,
) {
    let ParamValue::Color(v) = param.default_value else { return };
    let mut color = [v[0], v[1], v[2]];

    inline_property(ui, row, name, theme, |ui| {
        if ui.color_edit_button_rgb(&mut color).changed() {
            let n = name.to_string();
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                cmds.push(move |world: &mut World| {
                    let mut s = world.resource_mut::<ShaderEditorState>();
                    if let Some(p) = s.shader_file.params.get_mut(&n) {
                        p.default_value = ParamValue::Color([color[0], color[1], color[2], 1.0]);
                    }
                    s.is_modified = true;
                    drop(s);
                    reapply_params(world);
                });
            }
        }
    });
}

fn render_vec_param(
    ui: &mut egui::Ui,
    world: &World,
    name: &str,
    param: &renzora_shader::file::ShaderParam,
    theme: &renzora_theme::Theme,
    row: usize,
) {
    match param.default_value {
        ParamValue::Vec2(v) => {
            let mut val = v;
            inline_property(ui, row, name, theme, |ui| {
                let w = ((ui.available_width() - 32.0) / 2.0).max(30.0);
                ui.spacing_mut().item_spacing.x = 2.0;
                let mut changed = false;
                ui.label(RichText::new("X").size(10.0).color(egui::Color32::from_rgb(230, 90, 90)));
                changed |= ui.add_sized([w, 16.0], egui::DragValue::new(&mut val[0]).speed(0.01)).changed();
                ui.label(RichText::new("Y").size(10.0).color(egui::Color32::from_rgb(90, 200, 90)));
                changed |= ui.add_sized([w, 16.0], egui::DragValue::new(&mut val[1]).speed(0.01)).changed();
                if changed {
                    let n = name.to_string();
                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            let mut s = world.resource_mut::<ShaderEditorState>();
                            if let Some(p) = s.shader_file.params.get_mut(&n) {
                                p.default_value = ParamValue::Vec2(val);
                            }
                            s.is_modified = true;
                            drop(s);
                            reapply_params(world);
                        });
                    }
                }
            });
        }
        ParamValue::Vec3(v) => {
            let mut val = v;
            inline_property(ui, row, name, theme, |ui| {
                let w = ((ui.available_width() - 48.0) / 3.0).max(30.0);
                ui.spacing_mut().item_spacing.x = 2.0;
                let mut changed = false;
                ui.label(RichText::new("X").size(10.0).color(egui::Color32::from_rgb(230, 90, 90)));
                changed |= ui.add_sized([w, 16.0], egui::DragValue::new(&mut val[0]).speed(0.01)).changed();
                ui.label(RichText::new("Y").size(10.0).color(egui::Color32::from_rgb(90, 200, 90)));
                changed |= ui.add_sized([w, 16.0], egui::DragValue::new(&mut val[1]).speed(0.01)).changed();
                ui.label(RichText::new("Z").size(10.0).color(egui::Color32::from_rgb(90, 130, 230)));
                changed |= ui.add_sized([w, 16.0], egui::DragValue::new(&mut val[2]).speed(0.01)).changed();
                if changed {
                    let n = name.to_string();
                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            let mut s = world.resource_mut::<ShaderEditorState>();
                            if let Some(p) = s.shader_file.params.get_mut(&n) {
                                p.default_value = ParamValue::Vec3(val);
                            }
                            s.is_modified = true;
                            drop(s);
                            reapply_params(world);
                        });
                    }
                }
            });
        }
        ParamValue::Vec4(v) => {
            let mut val = v;
            inline_property(ui, row, name, theme, |ui| {
                let w = ((ui.available_width() - 64.0) / 4.0).max(25.0);
                ui.spacing_mut().item_spacing.x = 2.0;
                let mut changed = false;
                ui.label(RichText::new("X").size(10.0).color(egui::Color32::from_rgb(230, 90, 90)));
                changed |= ui.add_sized([w, 16.0], egui::DragValue::new(&mut val[0]).speed(0.01)).changed();
                ui.label(RichText::new("Y").size(10.0).color(egui::Color32::from_rgb(90, 200, 90)));
                changed |= ui.add_sized([w, 16.0], egui::DragValue::new(&mut val[1]).speed(0.01)).changed();
                ui.label(RichText::new("Z").size(10.0).color(egui::Color32::from_rgb(90, 130, 230)));
                changed |= ui.add_sized([w, 16.0], egui::DragValue::new(&mut val[2]).speed(0.01)).changed();
                ui.label(RichText::new("W").size(10.0).color(egui::Color32::from_rgb(200, 200, 90)));
                changed |= ui.add_sized([w, 16.0], egui::DragValue::new(&mut val[3]).speed(0.01)).changed();
                if changed {
                    let n = name.to_string();
                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            let mut s = world.resource_mut::<ShaderEditorState>();
                            if let Some(p) = s.shader_file.params.get_mut(&n) {
                                p.default_value = ParamValue::Vec4(val);
                            }
                            s.is_modified = true;
                            drop(s);
                            reapply_params(world);
                        });
                    }
                }
            });
        }
        _ => {}
    }
}

fn render_int_param(
    ui: &mut egui::Ui,
    world: &World,
    name: &str,
    param: &renzora_shader::file::ShaderParam,
    theme: &renzora_theme::Theme,
    row: usize,
) {
    let ParamValue::Int(v) = param.default_value else { return };
    let mut val = v;

    inline_property(ui, row, name, theme, |ui| {
        if ui.add(egui::DragValue::new(&mut val).speed(1)).changed() {
            let n = name.to_string();
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                cmds.push(move |world: &mut World| {
                    let mut s = world.resource_mut::<ShaderEditorState>();
                    if let Some(p) = s.shader_file.params.get_mut(&n) {
                        p.default_value = ParamValue::Int(val);
                    }
                    s.is_modified = true;
                    drop(s);
                    reapply_params(world);
                });
            }
        }
    });
}

fn render_bool_param(
    ui: &mut egui::Ui,
    world: &World,
    name: &str,
    param: &renzora_shader::file::ShaderParam,
    theme: &renzora_theme::Theme,
    row: usize,
) {
    let ParamValue::Bool(v) = param.default_value else { return };
    let mut val = v;

    inline_property(ui, row, name, theme, |ui| {
        if ui.checkbox(&mut val, "").changed() {
            let n = name.to_string();
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                cmds.push(move |world: &mut World| {
                    let mut s = world.resource_mut::<ShaderEditorState>();
                    if let Some(p) = s.shader_file.params.get_mut(&n) {
                        p.default_value = ParamValue::Bool(val);
                    }
                    s.is_modified = true;
                    drop(s);
                    reapply_params(world);
                });
            }
        }
    });
}
