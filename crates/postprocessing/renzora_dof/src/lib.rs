use bevy::prelude::*;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{AppEditorExt, EditorCommands, FieldDef, FieldType, FieldValue, InspectorEntry},
    renzora_theme::Theme,
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DepthOfFieldSettings {
    /// 0 = Gaussian, 1 = Bokeh
    pub mode: u32,
    pub focal_distance: f32,
    pub aperture_f_stops: f32,
    pub max_circle_of_confusion_diameter: f32,
    pub enabled: bool,
}

impl Default for DepthOfFieldSettings {
    fn default() -> Self {
        Self {
            mode: 0,
            focal_distance: 10.0,
            aperture_f_stops: 1.0,
            max_circle_of_confusion_diameter: 64.0,
            enabled: true,
        }
    }
}

fn sync_dof(
    mut commands: Commands,
    query: Query<(Entity, &DepthOfFieldSettings), Changed<DepthOfFieldSettings>>,
) {
    for (entity, settings) in &query {
        if settings.enabled {
            let mode = match settings.mode {
                1 => DepthOfFieldMode::Bokeh,
                _ => DepthOfFieldMode::Gaussian,
            };
            commands.entity(entity).insert(DepthOfField {
                mode,
                focal_distance: settings.focal_distance,
                sensor_height: 0.01866,
                aperture_f_stops: settings.aperture_f_stops,
                max_circle_of_confusion_diameter: settings.max_circle_of_confusion_diameter,
                max_depth: f32::INFINITY,
            });
        } else {
            commands.entity(entity).remove::<DepthOfField>();
        }
    }
}

#[cfg(feature = "editor")]
const DOF_MODE_LABELS: [&str; 2] = ["Gaussian", "Bokeh"];

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "depth_of_field",
        display_name: "Depth of Field",
        icon: regular::CAMERA,
        category: "rendering",
        has_fn: |world, entity| world.get::<DepthOfFieldSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(DepthOfFieldSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(DepthOfFieldSettings, DepthOfField)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<DepthOfFieldSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![
            FieldDef {
                name: "Focal Distance",
                field_type: FieldType::Float { speed: 0.1, min: 0.1, max: 1000.0 },
                get_fn: |world, entity| world.get::<DepthOfFieldSettings>(entity).map(|s| FieldValue::Float(s.focal_distance)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) { s.focal_distance = v; } } },
            },
            FieldDef {
                name: "Aperture (f-stops)",
                field_type: FieldType::Float { speed: 0.1, min: 0.1, max: 64.0 },
                get_fn: |world, entity| world.get::<DepthOfFieldSettings>(entity).map(|s| FieldValue::Float(s.aperture_f_stops)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) { s.aperture_f_stops = v; } } },
            },
            FieldDef {
                name: "Max CoC Diameter",
                field_type: FieldType::Float { speed: 1.0, min: 1.0, max: 256.0 },
                get_fn: |world, entity| world.get::<DepthOfFieldSettings>(entity).map(|s| FieldValue::Float(s.max_circle_of_confusion_diameter)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) { s.max_circle_of_confusion_diameter = v; } } },
            },
        ],
        custom_ui_fn: Some(dof_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn dof_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    _theme: &Theme,
) {
    let Some(settings) = world.get::<DepthOfFieldSettings>(entity) else {
        return;
    };
    let current = settings.mode as usize;
    let mut new_idx = current;
    ui.horizontal(|ui| {
        ui.label("Mode");
        egui::ComboBox::from_id_salt("dof_mode")
            .selected_text(*DOF_MODE_LABELS.get(current).unwrap_or(&"Unknown"))
            .show_ui(ui, |ui| {
                for (i, label) in DOF_MODE_LABELS.iter().enumerate() {
                    if ui.selectable_value(&mut new_idx, i, *label).changed() {
                        let mode = new_idx as u32;
                        cmds.push(move |world: &mut World| {
                            if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) {
                                s.mode = mode;
                            }
                        });
                    }
                }
            });
    });
}

fn cleanup_dof(mut commands: Commands, mut removed: RemovedComponents<DepthOfFieldSettings>) {
    for entity in removed.read() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<DepthOfField>();
        }
    }
}

pub struct DepthOfFieldPlugin;

impl Plugin for DepthOfFieldPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DepthOfFieldSettings>();
        app.add_systems(Update, (sync_dof, cleanup_dof));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
