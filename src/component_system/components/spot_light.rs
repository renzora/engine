//! Spot light component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;
use crate::component_system::SpotLightData;
use crate::ui::property_row;

use egui_phosphor::regular::FLASHLIGHT;

// ============================================================================
// Custom Add/Remove/Deserialize
// ============================================================================

fn add_spot_light(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let data = SpotLightData::default();
    commands.entity(entity).insert((
        SpotLight {
            color: Color::srgb(data.color.x, data.color.y, data.color.z),
            intensity: data.intensity,
            range: data.range,
            radius: data.radius,
            inner_angle: data.inner_angle,
            outer_angle: data.outer_angle,
            shadows_enabled: data.shadows_enabled,
            ..default()
        },
        data,
    ));
}

fn remove_spot_light(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<SpotLight>()
        .remove::<SpotLightData>();
}

fn deserialize_spot_light(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(sl_data) = serde_json::from_value::<SpotLightData>(data.clone()) {
        entity_commands.insert((
            SpotLight {
                color: Color::srgb(sl_data.color.x, sl_data.color.y, sl_data.color.z),
                intensity: sl_data.intensity,
                range: sl_data.range,
                radius: sl_data.radius,
                inner_angle: sl_data.inner_angle,
                outer_angle: sl_data.outer_angle,
                shadows_enabled: sl_data.shadows_enabled,
                ..default()
            },
            sl_data,
        ));
    }
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_spot_light(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<SpotLightData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgb(
                    (data.color.x * 255.0) as u8,
                    (data.color.y * 255.0) as u8,
                    (data.color.z * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.color = Vec3::new(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    // Intensity
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Intensity");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.intensity)
                            .speed(10.0)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Range
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Range");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.range)
                            .speed(0.1)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Inner Angle
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Inner Angle");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut inner_deg = data.inner_angle.to_degrees();
                if ui
                    .add(
                        egui::DragValue::new(&mut inner_deg)
                            .speed(1.0)
                            .range(0.0..=90.0)
                            .suffix("\u{00b0}"),
                    )
                    .changed()
                {
                    data.inner_angle = inner_deg.to_radians();
                    changed = true;
                }
            });
        });
    });

    // Outer Angle
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Outer Angle");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut outer_deg = data.outer_angle.to_degrees();
                if ui
                    .add(
                        egui::DragValue::new(&mut outer_deg)
                            .speed(1.0)
                            .range(0.0..=90.0)
                            .suffix("\u{00b0}"),
                    )
                    .changed()
                {
                    data.outer_angle = outer_deg.to_radians();
                    changed = true;
                }
            });
        });
    });

    // Shadows
    property_row(ui, 5, |ui| {
        ui.horizontal(|ui| {
            ui.label("Shadows");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.shadows_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Script Property Access
// ============================================================================

fn spot_light_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("intensity", PropertyValueType::Float),
        ("inner_angle", PropertyValueType::Float),
        ("outer_angle", PropertyValueType::Float),
        ("range", PropertyValueType::Float),
        ("color_r", PropertyValueType::Float),
        ("color_g", PropertyValueType::Float),
        ("color_b", PropertyValueType::Float),
        ("shadows_enabled", PropertyValueType::Bool),
    ]
}

fn spot_light_get_props(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<SpotLightData>(entity) else { return vec![] };
    vec![
        ("intensity", PropertyValue::Float(data.intensity)),
        ("inner_angle", PropertyValue::Float(data.inner_angle)),
        ("outer_angle", PropertyValue::Float(data.outer_angle)),
        ("range", PropertyValue::Float(data.range)),
        ("color_r", PropertyValue::Float(data.color.x)),
        ("color_g", PropertyValue::Float(data.color.y)),
        ("color_b", PropertyValue::Float(data.color.z)),
        ("shadows_enabled", PropertyValue::Bool(data.shadows_enabled)),
    ]
}

fn spot_light_set_prop(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<SpotLightData>(entity) else { return false };
    match prop {
        "intensity" => { if let PropertyValue::Float(v) = val { data.intensity = *v; true } else { false } }
        "inner_angle" => { if let PropertyValue::Float(v) = val { data.inner_angle = *v; true } else { false } }
        "outer_angle" => { if let PropertyValue::Float(v) = val { data.outer_angle = *v; true } else { false } }
        "range" => { if let PropertyValue::Float(v) = val { data.range = *v; true } else { false } }
        "shadows_enabled" => { if let PropertyValue::Bool(v) = val { data.shadows_enabled = *v; true } else { false } }
        _ => false,
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(SpotLightData {
        type_id: "spot_light",
        display_name: "Spot Light",
        category: ComponentCategory::Lighting,
        icon: FLASHLIGHT,
        priority: 2,
        conflicts_with: ["point_light", "directional_light"],
        custom_inspector: inspect_spot_light,
        custom_add: add_spot_light,
        custom_remove: remove_spot_light,
        custom_deserialize: deserialize_spot_light,
        custom_script_properties: spot_light_get_props,
        custom_script_set: spot_light_set_prop,
        custom_script_meta: spot_light_property_meta,
    }));
}
