//! Point light component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;
use crate::component_system::PointLightData;
use crate::ui::property_row;

use egui_phosphor::regular::LIGHTBULB;

// ============================================================================
// Custom Add/Remove/Deserialize
// ============================================================================

fn add_point_light(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let data = PointLightData::default();
    commands.entity(entity).insert((
        PointLight {
            color: Color::srgb(data.color.x, data.color.y, data.color.z),
            intensity: data.intensity,
            range: data.range,
            radius: data.radius,
            shadows_enabled: data.shadows_enabled,
            ..default()
        },
        data,
    ));
}

fn remove_point_light(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<PointLight>()
        .remove::<PointLightData>();
}

fn deserialize_point_light(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(pl_data) = serde_json::from_value::<PointLightData>(data.clone()) {
        entity_commands.insert((
            PointLight {
                color: Color::srgb(pl_data.color.x, pl_data.color.y, pl_data.color.z),
                intensity: pl_data.intensity,
                range: pl_data.range,
                radius: pl_data.radius,
                shadows_enabled: pl_data.shadows_enabled,
                ..default()
            },
            pl_data,
        ));
    }
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_point_light(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<PointLightData>(entity) else {
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

    // Shadows
    property_row(ui, 3, |ui| {
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

fn point_light_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("light_intensity", PropertyValueType::Float),
        ("light_range", PropertyValueType::Float),
        ("light_radius", PropertyValueType::Float),
        ("light_color_r", PropertyValueType::Float),
        ("light_color_g", PropertyValueType::Float),
        ("light_color_b", PropertyValueType::Float),
        ("shadows_enabled", PropertyValueType::Bool),
    ]
}

fn point_light_get_props(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<PointLightData>(entity) else { return vec![] };
    vec![
        ("light_intensity", PropertyValue::Float(data.intensity)),
        ("light_range", PropertyValue::Float(data.range)),
        ("light_radius", PropertyValue::Float(data.radius)),
        ("light_color_r", PropertyValue::Float(data.color.x)),
        ("light_color_g", PropertyValue::Float(data.color.y)),
        ("light_color_b", PropertyValue::Float(data.color.z)),
        ("shadows_enabled", PropertyValue::Bool(data.shadows_enabled)),
    ]
}

fn point_light_set_prop(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<PointLightData>(entity) else { return false };
    match prop {
        "light_intensity" => { if let PropertyValue::Float(v) = val { data.intensity = *v; true } else { false } }
        "light_range" => { if let PropertyValue::Float(v) = val { data.range = *v; true } else { false } }
        "light_radius" => { if let PropertyValue::Float(v) = val { data.radius = *v; true } else { false } }
        "shadows_enabled" => { if let PropertyValue::Bool(v) = val { data.shadows_enabled = *v; true } else { false } }
        _ => false,
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(PointLightData {
        type_id: "point_light",
        display_name: "Point Light",
        category: ComponentCategory::Lighting,
        icon: LIGHTBULB,
        priority: 0,
        conflicts_with: ["directional_light", "spot_light"],
        custom_inspector: inspect_point_light,
        custom_add: add_point_light,
        custom_remove: remove_point_light,
        custom_deserialize: deserialize_point_light,
        custom_script_properties: point_light_get_props,
        custom_script_set: point_light_set_prop,
        custom_script_meta: point_light_property_meta,
    }));
}
