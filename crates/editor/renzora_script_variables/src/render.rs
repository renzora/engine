use bevy_egui::egui::{self, RichText};
use renzora_scripting::{ScriptValue, ScriptVariableDefinition};
use renzora_theme::Theme;

use egui_phosphor::regular::{CODE, INFO};

/// Render the script variables panel content.
pub fn render_script_variables_content(
    ui: &mut egui::Ui,
    script_name: &str,
    props: &[ScriptVariableDefinition],
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();
    let disabled = theme.text.disabled.to_color32();
    let accent = theme.semantic.accent.to_color32();

    // No script open
    if script_name.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new(CODE).size(32.0).color(disabled));
            ui.add_space(10.0);
            ui.label(
                RichText::new("Open a script to see its variables")
                    .size(12.0)
                    .color(muted),
            );
        });
        return;
    }

    // Header
    ui.horizontal(|ui| {
        ui.label(RichText::new(CODE).size(14.0).color(accent));
        ui.label(RichText::new(script_name).size(13.0).strong());
    });
    ui.separator();

    if props.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("No variables defined").size(12.0).color(muted));
            ui.add_space(10.0);
            ui.label(
                RichText::new("Add a props() function to define variables:")
                    .size(11.0)
                    .color(disabled),
            );
            ui.add_space(6.0);

            let example = r#"fn props() {
    #{
        speed: #{ value: 5.0, hint: "Movement speed" },
        health: #{ value: 100 },
        name: #{ value: "Player" },
        active: #{ value: true },
    }
}"#;
            egui::Frame::new()
                .fill(theme.surfaces.extreme.to_color32())
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(example)
                            .size(10.0)
                            .color(muted)
                            .monospace(),
                    );
                });
        });
        return;
    }

    // Variable list
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (idx, prop) in props.iter().enumerate() {
            render_prop_row(ui, idx, prop, theme);
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(INFO).size(12.0).color(disabled));
            ui.label(
                RichText::new("Edit values in the Inspector panel")
                    .size(10.0)
                    .color(disabled),
            );
        });
    });
}

fn render_prop_row(
    ui: &mut egui::Ui,
    idx: usize,
    prop: &ScriptVariableDefinition,
    theme: &Theme,
) {
    let bg = if idx % 2 == 0 {
        theme.panels.inspector_row_even.to_color32()
    } else {
        theme.panels.inspector_row_odd.to_color32()
    };

    egui::Frame::new()
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                // Type badge
                let type_name = prop.default_value.type_name();
                ui.label(
                    RichText::new(type_name)
                        .size(10.0)
                        .color(type_color(type_name))
                        .monospace(),
                );

                // Variable name
                ui.label(RichText::new(&prop.display_name).size(11.0));

                // Default value (right-aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let default_str = format_default(&prop.default_value);
                    ui.label(
                        RichText::new(default_str)
                            .size(10.0)
                            .color(theme.text.muted.to_color32())
                            .monospace(),
                    );
                });
            });

            // Hint
            if let Some(ref hint) = prop.hint {
                ui.label(
                    RichText::new(hint)
                        .size(9.0)
                        .color(theme.text.disabled.to_color32())
                        .italics(),
                );
            }
        });
}

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

fn format_default(value: &ScriptValue) -> String {
    match value {
        ScriptValue::Float(v) => format!("{:.2}", v),
        ScriptValue::Int(v) => format!("{}", v),
        ScriptValue::Bool(v) => format!("{}", v),
        ScriptValue::String(v) => format!("\"{}\"", v),
        ScriptValue::Entity(v) => {
            if v.is_empty() {
                "(none)".to_string()
            } else {
                v.clone()
            }
        }
        ScriptValue::Vec2(v) => format!("({:.1}, {:.1})", v.x, v.y),
        ScriptValue::Vec3(v) => format!("({:.1}, {:.1}, {:.1})", v.x, v.y, v.z),
        ScriptValue::Color(c) => format!("rgba({:.2}, {:.2}, {:.2}, {:.2})", c.x, c.y, c.z, c.w),
    }
}
