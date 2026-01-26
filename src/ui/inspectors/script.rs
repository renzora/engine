//! Inspector widget for script components

use std::path::PathBuf;
use bevy_egui::egui::{self, Color32, RichText, CornerRadius, Margin};

use crate::scripting::{ScriptComponent, ScriptRegistry, ScriptValue, RhaiScriptEngine};

/// Render a section header with background
fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    egui::Frame::NONE
        .fill(Color32::from_rgb(40, 40, 48))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).color(Color32::from_rgb(180, 180, 190)).strong());
        });
    ui.add_space(4.0);
}

/// Render the script component inspector
pub fn render_script_inspector(
    ui: &mut egui::Ui,
    script: &mut ScriptComponent,
    registry: &ScriptRegistry,
    rhai_engine: &RhaiScriptEngine,
) -> bool {
    let mut changed = false;

    section_header(ui, "Script");

    // Determine current mode
    let is_file_script = script.script_path.is_some();

    // Get available file scripts
    let available_files = rhai_engine.get_available_scripts();

    // Script source selector
    ui.horizontal(|ui| {
        ui.label("Source");

        let current_source = if is_file_script { "File" } else { "Built-in" };

        egui::ComboBox::from_id_salt("script_source_selector")
            .selected_text(current_source)
            .show_ui(ui, |ui| {
                if ui.selectable_label(!is_file_script, "Built-in").clicked() {
                    script.script_path = None;
                    script.runtime_state.initialized = false;
                    changed = true;
                }
                if ui.selectable_label(is_file_script, "File").clicked() {
                    script.script_id = String::new();
                    script.script_path = Some(PathBuf::new());
                    script.runtime_state.initialized = false;
                    changed = true;
                }
            });
    });

    ui.add_space(4.0);

    if is_file_script {
        // File-based script selector
        ui.horizontal(|ui| {
            ui.label("Script File");

            let current_name = script
                .script_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("<Select Script>");

            egui::ComboBox::from_id_salt("script_file_selector")
                .selected_text(current_name)
                .show_ui(ui, |ui| {
                    if available_files.is_empty() {
                        ui.label(RichText::new("No scripts found").color(Color32::from_rgb(150, 150, 160)).italics());
                        ui.label(RichText::new("Create .rhai files in project/scripts/").color(Color32::from_rgb(120, 120, 130)).small());
                    } else {
                        for (name, path) in &available_files {
                            let selected = script.script_path.as_ref() == Some(path);
                            if ui.selectable_label(selected, name).clicked() {
                                script.script_path = Some(path.clone());
                                script.variables = Default::default();
                                script.runtime_state.initialized = false;
                                changed = true;
                            }
                        }
                    }
                });
        });

        // Show file path
        if let Some(path) = &script.script_path {
            if path.as_os_str().is_empty() {
                ui.label(RichText::new("No script selected").color(Color32::from_rgb(200, 150, 100)).small());
            } else {
                ui.label(RichText::new(format!("Path: {}", path.display())).color(Color32::from_rgb(100, 100, 110)).small());
            }
        }

        // Hot reload indicator
        ui.add_space(4.0);
        ui.label(RichText::new("Scripts auto-reload on file change").color(Color32::from_rgb(100, 160, 100)).small().italics());

        // Show props for file-based scripts
        if let Some(path) = &script.script_path {
            if !path.as_os_str().is_empty() {
                let props = rhai_engine.get_script_props(path);
                if !props.is_empty() {
                    ui.add_space(8.0);
                    ui.label(RichText::new("Properties").color(Color32::from_rgb(153, 153, 166)));
                    ui.separator();
                    ui.add_space(4.0);

                    for prop in &props {
                        // Ensure variable exists with default value
                        if script.variables.get(&prop.name).is_none() {
                            script.variables.set(&prop.name, prop.default_value.clone());
                        }

                        ui.horizontal(|ui| {
                            ui.label(&prop.display_name);

                            // Get mutable reference to the value
                            if let Some(value) = script.variables.get_mut(&prop.name) {
                                match value {
                                    ScriptValue::Float(ref mut v) => {
                                        if ui.add(egui::DragValue::new(v).speed(0.1)).changed() {
                                            changed = true;
                                        }
                                    }
                                    ScriptValue::Int(ref mut v) => {
                                        if ui.add(egui::DragValue::new(v)).changed() {
                                            changed = true;
                                        }
                                    }
                                    ScriptValue::Bool(ref mut v) => {
                                        if ui.checkbox(v, "").changed() {
                                            changed = true;
                                        }
                                    }
                                    ScriptValue::String(ref mut v) => {
                                        if ui.text_edit_singleline(v).changed() {
                                            changed = true;
                                        }
                                    }
                                    ScriptValue::Vec2(ref mut v) => {
                                        ui.horizontal(|ui| {
                                            ui.label("X");
                                            if ui.add(egui::DragValue::new(&mut v.x).speed(0.1)).changed() {
                                                changed = true;
                                            }
                                            ui.label("Y");
                                            if ui.add(egui::DragValue::new(&mut v.y).speed(0.1)).changed() {
                                                changed = true;
                                            }
                                        });
                                    }
                                    ScriptValue::Vec3(ref mut v) => {
                                        if ui.add(egui::DragValue::new(&mut v.x).speed(0.1).prefix("X: ")).changed() {
                                            changed = true;
                                        }
                                        if ui.add(egui::DragValue::new(&mut v.y).speed(0.1).prefix("Y: ")).changed() {
                                            changed = true;
                                        }
                                        if ui.add(egui::DragValue::new(&mut v.z).speed(0.1).prefix("Z: ")).changed() {
                                            changed = true;
                                        }
                                    }
                                    ScriptValue::Color(ref mut v) => {
                                        let mut color = egui::Color32::from_rgba_unmultiplied(
                                            (v.x * 255.0) as u8,
                                            (v.y * 255.0) as u8,
                                            (v.z * 255.0) as u8,
                                            (v.w * 255.0) as u8,
                                        );
                                        if ui.color_edit_button_srgba(&mut color).changed() {
                                            *v = bevy::prelude::Vec4::new(
                                                color.r() as f32 / 255.0,
                                                color.g() as f32 / 255.0,
                                                color.b() as f32 / 255.0,
                                                color.a() as f32 / 255.0,
                                            );
                                            changed = true;
                                        }
                                    }
                                }
                            }
                        });

                        // Show hint if available
                        if let Some(hint) = &prop.hint {
                            ui.label(RichText::new(hint).color(Color32::from_rgb(100, 100, 110)).small());
                        }
                    }
                }
            }
        }
    } else {
        // Built-in script selector
        ui.horizontal(|ui| {
            ui.label("Script");

            let current_name = registry
                .get(&script.script_id)
                .map(|s| s.name())
                .unwrap_or("<None>");

            egui::ComboBox::from_id_salt("script_selector")
                .selected_text(current_name)
                .show_ui(ui, |ui| {
                    // None option
                    if ui.selectable_label(script.script_id.is_empty(), "<None>").clicked() {
                        script.script_id = String::new();
                        script.variables = Default::default();
                        script.runtime_state.initialized = false;
                        changed = true;
                    }

                    ui.separator();

                    // Group by category
                    let mut categories: Vec<_> = registry.categories().collect();
                    categories.sort();

                    for category in categories {
                        ui.label(RichText::new(category.as_str()).color(Color32::from_rgb(130, 130, 145)).small());

                        if let Some(script_ids) = registry.by_category(category) {
                            for id in script_ids {
                                if let Some(s) = registry.get(id) {
                                    let selected = script.script_id == *id;
                                    if ui.selectable_label(selected, s.name()).clicked() {
                                        script.script_id = id.clone();
                                        // Reset variables to defaults for new script
                                        script.variables = Default::default();
                                        for var_def in s.variables() {
                                            script.variables.set(&var_def.name, var_def.default_value.clone());
                                        }
                                        script.runtime_state.initialized = false;
                                        changed = true;
                                    }
                                }
                            }
                        }
                        ui.add_space(4.0);
                    }
                });
        });

        // Show script info for built-in scripts
        if let Some(s) = registry.get(&script.script_id) {
            if !s.description().is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(s.description()).color(Color32::from_rgb(120, 120, 130)).italics().small());
            }

            // Variables section
            let var_defs = s.variables();
            if !var_defs.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new("Variables").color(Color32::from_rgb(153, 153, 166)));
                ui.separator();
                ui.add_space(4.0);

                for var_def in var_defs {
                    // Ensure variable exists with default value
                    if script.variables.get(&var_def.name).is_none() {
                        script.variables.set(&var_def.name, var_def.default_value.clone());
                    }

                    ui.horizontal(|ui| {
                        ui.label(&var_def.display_name);

                        // Get mutable reference to the value
                        if let Some(value) = script.variables.get_mut(&var_def.name) {
                            match value {
                                ScriptValue::Float(ref mut v) => {
                                    if ui.add(egui::DragValue::new(v).speed(0.1)).changed() {
                                        changed = true;
                                    }
                                }
                                ScriptValue::Int(ref mut v) => {
                                    if ui.add(egui::DragValue::new(v)).changed() {
                                        changed = true;
                                    }
                                }
                                ScriptValue::Bool(ref mut v) => {
                                    if ui.checkbox(v, "").changed() {
                                        changed = true;
                                    }
                                }
                                ScriptValue::String(ref mut v) => {
                                    if ui.text_edit_singleline(v).changed() {
                                        changed = true;
                                    }
                                }
                                ScriptValue::Vec2(ref mut v) => {
                                    ui.horizontal(|ui| {
                                        ui.label("X");
                                        if ui.add(egui::DragValue::new(&mut v.x).speed(0.1)).changed() {
                                            changed = true;
                                        }
                                        ui.label("Y");
                                        if ui.add(egui::DragValue::new(&mut v.y).speed(0.1)).changed() {
                                            changed = true;
                                        }
                                    });
                                }
                                ScriptValue::Vec3(ref mut v) => {
                                    if ui.add(egui::DragValue::new(&mut v.x).speed(0.1).prefix("X: ")).changed() {
                                        changed = true;
                                    }
                                    if ui.add(egui::DragValue::new(&mut v.y).speed(0.1).prefix("Y: ")).changed() {
                                        changed = true;
                                    }
                                    if ui.add(egui::DragValue::new(&mut v.z).speed(0.1).prefix("Z: ")).changed() {
                                        changed = true;
                                    }
                                }
                                ScriptValue::Color(ref mut v) => {
                                    let mut color = egui::Color32::from_rgba_unmultiplied(
                                        (v.x * 255.0) as u8,
                                        (v.y * 255.0) as u8,
                                        (v.z * 255.0) as u8,
                                        (v.w * 255.0) as u8,
                                    );
                                    if ui.color_edit_button_srgba(&mut color).changed() {
                                        *v = bevy::prelude::Vec4::new(
                                            color.r() as f32 / 255.0,
                                            color.g() as f32 / 255.0,
                                            color.b() as f32 / 255.0,
                                            color.a() as f32 / 255.0,
                                        );
                                        changed = true;
                                    }
                                }
                            }
                        }
                    });

                    // Show hint if available
                    if let Some(hint) = &var_def.hint {
                        ui.label(RichText::new(hint).color(Color32::from_rgb(100, 100, 110)).small());
                    }
                }
            }
        }
    }

    // Enabled checkbox
    ui.add_space(8.0);
    if ui.checkbox(&mut script.enabled, "Enabled").changed() {
        changed = true;
    }

    ui.add_space(4.0);

    changed
}
