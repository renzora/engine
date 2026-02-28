//! Script Variables panel - shows props from the active script in the Script Editor
//!
//! Automatically updates as the user edits code, providing live feedback
//! on defined variables without needing to save the file.

use bevy_egui::egui;

use crate::core::SceneManagerState;
use crate::scripting::{RhaiScriptEngine, ScriptVariableDefinition};
use crate::project::CurrentProject;

use egui_phosphor::regular::{CODE, INFO};

/// Render the Script Variables panel content
pub fn render_script_variables_content(
    ui: &mut egui::Ui,
    scene_state: &SceneManagerState,
    rhai_engine: &RhaiScriptEngine,
    _current_project: Option<&CurrentProject>,
) {
    // Get the active script tab
    let active_script = scene_state.active_script_tab
        .and_then(|idx| scene_state.open_scripts.get(idx));

    let Some(script) = active_script else {
        // No script open - show placeholder
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                egui::RichText::new(CODE)
                    .size(32.0)
                    .color(egui::Color32::from_gray(80)),
            );
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("Open a script to see its variables")
                    .size(12.0)
                    .color(egui::Color32::from_gray(100)),
            );
        });
        return;
    };

    // Parse props from the in-memory content (live updates as user types)
    let props = rhai_engine.get_props_from_source(&script.content);

    // Header: script name
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(CODE)
                .size(14.0)
                .color(egui::Color32::from_rgb(120, 180, 255)),
        );
        ui.label(
            egui::RichText::new(&script.name)
                .size(13.0)
                .strong(),
        );
    });
    ui.separator();

    if props.is_empty() {
        // No props defined
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("No variables defined")
                    .size(12.0)
                    .color(egui::Color32::from_gray(100)),
            );
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("Add a fn props() to define variables:")
                    .size(11.0)
                    .color(egui::Color32::from_gray(80)),
            );
            ui.add_space(6.0);

            // Example code
            let example = r#"fn props() {
    #{
        speed: #{ value: 5.0, hint: "Movement speed" },
        health: #{ value: 100 },
        name: #{ value: "Player" },
        active: #{ value: true },
    }
}"#;
            egui::Frame::new()
                .fill(egui::Color32::from_gray(25))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(example)
                            .size(10.0)
                            .color(egui::Color32::from_gray(140))
                            .monospace(),
                    );
                });
        });
        return;
    }

    // Render each prop
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (idx, prop) in props.iter().enumerate() {
            render_prop_row(ui, idx, prop);
        }

        // Hint section
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(INFO)
                    .size(12.0)
                    .color(egui::Color32::from_gray(80)),
            );
            ui.label(
                egui::RichText::new("Edit values in the Inspector panel")
                    .size(10.0)
                    .color(egui::Color32::from_gray(80)),
            );
        });
    });
}

/// Render a single prop definition row (read-only display)
fn render_prop_row(ui: &mut egui::Ui, idx: usize, prop: &ScriptVariableDefinition) {
    let bg_color = if idx % 2 == 0 {
        egui::Color32::from_gray(32)
    } else {
        egui::Color32::from_gray(36)
    };

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                // Type badge
                let type_name = prop.default_value.type_name();
                let type_color = type_color(type_name);
                ui.label(
                    egui::RichText::new(type_name)
                        .size(10.0)
                        .color(type_color)
                        .monospace(),
                );

                // Variable name
                ui.label(
                    egui::RichText::new(&prop.display_name)
                        .size(11.0),
                );

                // Default value (right-aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let default_str = format_default(&prop.default_value);
                    ui.label(
                        egui::RichText::new(default_str)
                            .size(10.0)
                            .color(egui::Color32::from_gray(120))
                            .monospace(),
                    );
                });
            });

            // Show hint if available
            if let Some(ref hint) = prop.hint {
                ui.label(
                    egui::RichText::new(hint)
                        .size(9.0)
                        .color(egui::Color32::from_gray(90))
                        .italics(),
                );
            }
        });
}

/// Get a color for a type name
fn type_color(type_name: &str) -> egui::Color32 {
    match type_name {
        "Float" => egui::Color32::from_rgb(120, 200, 120),
        "Int" => egui::Color32::from_rgb(100, 180, 220),
        "Bool" => egui::Color32::from_rgb(220, 160, 100),
        "String" => egui::Color32::from_rgb(200, 140, 180),
        "Entity" => egui::Color32::from_rgb(100, 210, 200),
        "Vec2" => egui::Color32::from_rgb(180, 140, 220),
        "Vec3" => egui::Color32::from_rgb(140, 180, 220),
        "Color" => egui::Color32::from_rgb(220, 180, 100),
        _ => egui::Color32::from_gray(150),
    }
}

/// Format a default value for display
fn format_default(value: &crate::scripting::ScriptValue) -> String {
    use crate::scripting::ScriptValue;
    match value {
        ScriptValue::Float(v) => format!("{:.2}", v),
        ScriptValue::Int(v) => format!("{}", v),
        ScriptValue::Bool(v) => format!("{}", v),
        ScriptValue::String(v) => format!("\"{}\"", v),
        ScriptValue::Entity(v) => if v.is_empty() { "(none)".to_string() } else { v.clone() },
        ScriptValue::Vec2(v) => format!("({:.1}, {:.1})", v.x, v.y),
        ScriptValue::Vec3(v) => format!("({:.1}, {:.1}, {:.1})", v.x, v.y, v.z),
        ScriptValue::Color(c) => format!("rgba({:.2}, {:.2}, {:.2}, {:.2})", c.x, c.y, c.z, c.w),
    }
}
