//! Generic field rendering — maps FieldType to the appropriate egui widget.

use bevy::prelude::*;
use renzora::bevy_egui::egui;
use renzora::editor::{
    asset_drop_target, inline_property, toggle_switch, AssetDragPayload, EditorCommands,
    FieldDef, FieldType, FieldValue,
};
use renzora::theme::Theme;

/// Render a single field row using the appropriate widget for its type.
///
/// Reads the current value via `field.get_fn`, renders an editable widget,
/// and if the value changed, pushes a deferred write via `EditorCommands`.
pub fn render_field(
    ui: &mut egui::Ui,
    field: &FieldDef,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
    row_index: usize,
) {
    let Some(value) = (field.get_fn)(world, entity) else {
        return;
    };

    match (&field.field_type, value) {
        (FieldType::Float { speed, min, max }, FieldValue::Float(mut v)) => {
            let speed = *speed;
            let min = *min;
            let max = *max;
            let set_fn = field.set_fn;

            inline_property(ui, row_index, field.name, theme, |ui| {
                let orig = v;
                ui.add(
                    egui::DragValue::new(&mut v)
                        .speed(speed)
                        .range(min..=max),
                );
                if v != orig {
                    cmds.push(move |w| (set_fn)(w, entity, FieldValue::Float(v)));
                }
            });
        }

        (FieldType::Vec3 { speed }, FieldValue::Vec3(mut v)) => {
            let speed = *speed;
            let set_fn = field.set_fn;

            inline_property(ui, row_index, field.name, theme, |ui| {
                let orig = v;
                let w = ((ui.available_width() - 48.0) / 3.0).max(30.0);

                ui.spacing_mut().item_spacing.x = 2.0;

                // X
                ui.label(
                    egui::RichText::new("X")
                        .size(10.0)
                        .color(egui::Color32::from_rgb(230, 90, 90)),
                );
                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v[0]).speed(speed));

                // Y
                ui.label(
                    egui::RichText::new("Y")
                        .size(10.0)
                        .color(egui::Color32::from_rgb(130, 200, 90)),
                );
                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v[1]).speed(speed));

                // Z
                ui.label(
                    egui::RichText::new("Z")
                        .size(10.0)
                        .color(egui::Color32::from_rgb(90, 150, 230)),
                );
                ui.add_sized([w, 16.0], egui::DragValue::new(&mut v[2]).speed(speed));

                if v != orig {
                    cmds.push(move |w| (set_fn)(w, entity, FieldValue::Vec3(v)));
                }
            });
        }

        (FieldType::Bool, FieldValue::Bool(v)) => {
            let set_fn = field.set_fn;

            inline_property(ui, row_index, field.name, theme, |ui| {
                let id = ui.id().with(field.name);
                if toggle_switch(ui, id, v) {
                    let new_val = !v;
                    cmds.push(move |w| (set_fn)(w, entity, FieldValue::Bool(new_val)));
                }
            });
        }

        (FieldType::Color, FieldValue::Color(mut rgb)) => {
            let set_fn = field.set_fn;

            inline_property(ui, row_index, field.name, theme, |ui| {
                let orig = rgb;
                if ui.color_edit_button_rgb(&mut rgb).changed() && rgb != orig {
                    cmds.push(move |w| (set_fn)(w, entity, FieldValue::Color(rgb)));
                }
            });
        }

        (FieldType::String, FieldValue::String(mut s)) => {
            let set_fn = field.set_fn;

            inline_property(ui, row_index, field.name, theme, |ui| {
                let orig = s.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut s)
                        .desired_width(ui.available_width()),
                );
                if s != orig {
                    cmds.push(move |w| (set_fn)(w, entity, FieldValue::String(s)));
                }
            });
        }

        (FieldType::ReadOnly, FieldValue::ReadOnly(text)) => {
            inline_property(ui, row_index, field.name, theme, |ui| {
                ui.label(
                    egui::RichText::new(&text)
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                );
            });
        }

        (FieldType::Asset { extensions }, FieldValue::Asset(current)) => {
            let set_fn = field.set_fn;
            let extensions = extensions.clone();

            inline_property(ui, row_index, field.name, theme, |ui| {
                let payload = world.get_resource::<AssetDragPayload>();
                let ext_refs: Vec<&str> = extensions.iter().map(|s| s.as_str()).collect();
                let current_str = current.as_deref();

                let drop_result = asset_drop_target(
                    ui,
                    ui.id().with(field.name),
                    current_str,
                    &ext_refs,
                    "Drag asset here",
                    theme,
                    payload,
                );

                if let Some(path) = drop_result.dropped_path {
                    let path_str = if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
                        project.make_asset_relative(&path)
                    } else {
                        path.to_string_lossy().to_string()
                    };
                    cmds.push(move |w| (set_fn)(w, entity, FieldValue::Asset(Some(path_str))));
                }
                if drop_result.cleared {
                    cmds.push(move |w| (set_fn)(w, entity, FieldValue::Asset(None)));
                }
            });
        }

        _ => {} // mismatched type/value — skip
    }
}
