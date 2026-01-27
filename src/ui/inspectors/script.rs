//! Inspector widget for script components

use std::path::PathBuf;
use bevy_egui::egui::{self, Color32, RichText, Sense};

use crate::scripting::{ScriptComponent, ScriptRegistry, ScriptValue, RhaiScriptEngine};
use crate::ui::inline_property;

/// Render a collapsible section header
fn section_header(ui: &mut egui::Ui, id: &str, title: &str, default_open: bool) -> bool {
    let id = ui.make_persistent_id(id);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);

    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::click());
    if response.clicked() {
        state.toggle(ui);
    }

    let bg_color = if response.hovered() {
        Color32::from_rgb(50, 53, 60)
    } else {
        Color32::from_rgb(45, 48, 55)
    };

    ui.painter().rect_filled(rect, 0.0, bg_color);
    ui.painter().text(
        rect.left_center() + egui::vec2(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.0),
        Color32::from_rgb(180, 180, 190),
    );

    state.store(ui.ctx());
    state.is_open()
}

/// Render the script component inspector
pub fn render_script_inspector(
    ui: &mut egui::Ui,
    script: &mut ScriptComponent,
    registry: &ScriptRegistry,
    rhai_engine: &RhaiScriptEngine,
) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Determine current mode
    let is_file_script = script.script_path.is_some();

    // Get available file scripts
    let available_files = rhai_engine.get_available_scripts();

    // Script source selector
    changed |= inline_property(ui, row, "Source", |ui| {
        let current_source = if is_file_script { "File" } else { "Built-in" };
        let mut c = false;

        egui::ComboBox::from_id_salt("script_source_selector")
            .width(80.0)
            .selected_text(current_source)
            .show_ui(ui, |ui| {
                if ui.selectable_label(!is_file_script, "Built-in").clicked() {
                    script.script_path = None;
                    script.runtime_state.initialized = false;
                    c = true;
                }
                if ui.selectable_label(is_file_script, "File").clicked() {
                    script.script_id = String::new();
                    script.script_path = Some(PathBuf::new());
                    script.runtime_state.initialized = false;
                    c = true;
                }
            });
        c
    });
    row += 1;

    if is_file_script {
        // File-based script selector
        changed |= inline_property(ui, row, "Script File", |ui| {
            let current_name = script
                .script_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("<Select>");
            let mut c = false;

            egui::ComboBox::from_id_salt("script_file_selector")
                .width(100.0)
                .selected_text(current_name)
                .show_ui(ui, |ui| {
                    if available_files.is_empty() {
                        ui.label(RichText::new("No scripts found").color(Color32::from_rgb(150, 150, 160)).italics().size(11.0));
                    } else {
                        for (name, path) in &available_files {
                            let selected = script.script_path.as_ref() == Some(path);
                            if ui.selectable_label(selected, name).clicked() {
                                script.script_path = Some(path.clone());
                                script.variables = Default::default();
                                script.runtime_state.initialized = false;
                                c = true;
                            }
                        }
                    }
                });
            c
        });
        row += 1;

        // Show file path as info
        if let Some(path) = &script.script_path {
            if !path.as_os_str().is_empty() {
                inline_property(ui, row, "Path", |ui| {
                    ui.label(RichText::new(format!("{}", path.display())).color(Color32::from_rgb(100, 100, 110)).size(10.0));
                });
                row += 1;
            }
        }

        // Show props for file-based scripts
        if let Some(path) = &script.script_path {
            if !path.as_os_str().is_empty() {
                let props = rhai_engine.get_script_props(path);
                if !props.is_empty() {
                    ui.add_space(4.0);
                    if section_header(ui, "script_props", "Properties", true) {
                        for prop in &props {
                            // Ensure variable exists with default value
                            if script.variables.get(&prop.name).is_none() {
                                script.variables.set(&prop.name, prop.default_value.clone());
                            }

                            changed |= render_script_value(ui, row, &prop.display_name, script.variables.get_mut(&prop.name));
                            row += 1;

                            // Show hint if available
                            if let Some(hint) = &prop.hint {
                                inline_property(ui, row, "", |ui| {
                                    ui.label(RichText::new(hint).color(Color32::from_rgb(100, 100, 110)).size(10.0));
                                });
                                row += 1;
                            }
                        }
                    }
                }
            }
        }
    } else {
        // Built-in script selector
        changed |= inline_property(ui, row, "Script", |ui| {
            let current_name = registry
                .get(&script.script_id)
                .map(|s| s.name())
                .unwrap_or("<None>");
            let mut c = false;

            egui::ComboBox::from_id_salt("script_selector")
                .width(100.0)
                .selected_text(current_name)
                .show_ui(ui, |ui| {
                    // None option
                    if ui.selectable_label(script.script_id.is_empty(), "<None>").clicked() {
                        script.script_id = String::new();
                        script.variables = Default::default();
                        script.runtime_state.initialized = false;
                        c = true;
                    }

                    ui.separator();

                    // Group by category
                    let mut categories: Vec<_> = registry.categories().collect();
                    categories.sort();

                    for category in categories {
                        ui.label(RichText::new(category.as_str()).color(Color32::from_rgb(130, 130, 145)).size(10.0));

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
                                        c = true;
                                    }
                                }
                            }
                        }
                    }
                });
            c
        });
        row += 1;

        // Show script description
        if let Some(s) = registry.get(&script.script_id) {
            if !s.description().is_empty() {
                inline_property(ui, row, "", |ui| {
                    ui.label(RichText::new(s.description()).color(Color32::from_rgb(120, 120, 130)).italics().size(10.0));
                });
                row += 1;
            }

            // Variables section
            let var_defs = s.variables();
            if !var_defs.is_empty() {
                ui.add_space(4.0);
                if section_header(ui, "script_vars", "Variables", true) {
                    for var_def in var_defs {
                        // Ensure variable exists with default value
                        if script.variables.get(&var_def.name).is_none() {
                            script.variables.set(&var_def.name, var_def.default_value.clone());
                        }

                        changed |= render_script_value(ui, row, &var_def.display_name, script.variables.get_mut(&var_def.name));
                        row += 1;

                        // Show hint if available
                        if let Some(hint) = &var_def.hint {
                            inline_property(ui, row, "", |ui| {
                                ui.label(RichText::new(hint).color(Color32::from_rgb(100, 100, 110)).size(10.0));
                            });
                            row += 1;
                        }
                    }
                }
            }
        }
    }

    // Enabled checkbox
    ui.add_space(4.0);
    changed |= inline_property(ui, row, "Enabled", |ui| {
        ui.checkbox(&mut script.enabled, "").changed()
    });

    changed
}

/// Render a script value with inline_property style
fn render_script_value(ui: &mut egui::Ui, row: usize, label: &str, value: Option<&mut ScriptValue>) -> bool {
    let mut changed = false;

    if let Some(value) = value {
        match value {
            ScriptValue::Float(ref mut v) => {
                changed |= inline_property(ui, row, label, |ui| {
                    ui.add(egui::DragValue::new(v).speed(0.1)).changed()
                });
            }
            ScriptValue::Int(ref mut v) => {
                changed |= inline_property(ui, row, label, |ui| {
                    ui.add(egui::DragValue::new(v)).changed()
                });
            }
            ScriptValue::Bool(ref mut v) => {
                changed |= inline_property(ui, row, label, |ui| {
                    ui.checkbox(v, "").changed()
                });
            }
            ScriptValue::String(ref mut v) => {
                changed |= inline_property(ui, row, label, |ui| {
                    ui.add(egui::TextEdit::singleline(v).desired_width(100.0)).changed()
                });
            }
            ScriptValue::Vec2(ref mut v) => {
                changed |= inline_property(ui, row, label, |ui| {
                    let mut c = false;
                    if ui.add(egui::DragValue::new(&mut v.x).speed(0.1).prefix("X ")).changed() {
                        c = true;
                    }
                    if ui.add(egui::DragValue::new(&mut v.y).speed(0.1).prefix("Y ")).changed() {
                        c = true;
                    }
                    c
                });
            }
            ScriptValue::Vec3(ref mut v) => {
                changed |= inline_property(ui, row, label, |ui| {
                    let mut c = false;
                    if ui.add(egui::DragValue::new(&mut v.x).speed(0.1).prefix("X ")).changed() {
                        c = true;
                    }
                    if ui.add(egui::DragValue::new(&mut v.y).speed(0.1).prefix("Y ")).changed() {
                        c = true;
                    }
                    if ui.add(egui::DragValue::new(&mut v.z).speed(0.1).prefix("Z ")).changed() {
                        c = true;
                    }
                    c
                });
            }
            ScriptValue::Color(ref mut v) => {
                changed |= inline_property(ui, row, label, |ui| {
                    let mut color = egui::Color32::from_rgba_unmultiplied(
                        (v.x * 255.0) as u8,
                        (v.y * 255.0) as u8,
                        (v.z * 255.0) as u8,
                        (v.w * 255.0) as u8,
                    );
                    let c = ui.color_edit_button_srgba(&mut color).changed();
                    if c {
                        *v = bevy::prelude::Vec4::new(
                            color.r() as f32 / 255.0,
                            color.g() as f32 / 255.0,
                            color.b() as f32 / 255.0,
                            color.a() as f32 / 255.0,
                        );
                    }
                    c
                });
            }
        }
    }

    changed
}
