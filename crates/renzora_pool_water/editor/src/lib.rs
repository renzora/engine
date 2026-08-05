//! Editor companion to [`renzora_pool_water`] — the Pool Water inspector.
//!
//! Split out for the reason given in `renzora_forward_decal_editor`: the
//! inspector was behind `cfg(feature = "editor")` but its dependency on
//! `renzora_editor_framework` was not optional, so the editor framework
//! compiled into every shipped game. A feature could not have separated them
//! either — cargo unifies features across a `--workspace` build.

use bevy::prelude::*;
use renzora_editor_framework::AppEditorExt;
use renzora_pool_water::PoolWater;

fn pool_water_inspector_entry() -> renzora_editor_framework::InspectorEntry {
    use renzora_editor_framework::{FieldDef, FieldType, FieldValue, InspectorEntry};

    InspectorEntry {
        type_id: "pool_water",
        display_name: "Pool Water",
        icon: "swimming-pool",
        category: "rendering",
        has_fn: |world, entity| world.get::<PoolWater>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(PoolWater::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<PoolWater>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Water Level",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 0.5,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.water_level))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.water_level = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "IOR",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 1.0,
                    max: 2.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.ior))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.ior = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Fresnel Min",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.fresnel_min))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.fresnel_min = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Caustic Intensity",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 2.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.caustic_intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.caustic_intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Deep Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Color(s.deep_color))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.deep_color = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Shallow Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Color(s.shallow_color))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.shallow_color = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Foam Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Color(s.foam_color))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.foam_color = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Refraction Strength",
                field_type: FieldType::Float {
                    speed: 0.005,
                    min: 0.0,
                    max: 0.2,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.refraction_strength))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.refraction_strength = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Max Depth",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.5,
                    max: 50.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.max_depth))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.max_depth = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Foam Depth",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 5.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.foam_depth))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.foam_depth = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Damping",
                field_type: FieldType::Float {
                    speed: 0.001,
                    min: 0.9,
                    max: 0.999,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.damping))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.damping = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Wave Speed",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.1,
                    max: 5.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.wave_speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.wave_speed = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Height Scale",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.01,
                    max: 2.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.height_scale))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.height_scale = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Specular Power",
                field_type: FieldType::Float {
                    speed: 100.0,
                    min: 100.0,
                    max: 10000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PoolWater>(entity)
                        .map(|s| FieldValue::Float(s.specular_power))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<PoolWater>(entity) {
                            s.specular_power = v;
                        }
                    }
                },
            },
        ],
    }
}

#[derive(Default)]
pub struct PoolWaterEditorPlugin;

impl Plugin for PoolWaterEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] PoolWaterEditorPlugin");
        app.register_inspector(pool_water_inspector_entry());
    }
}

renzora::add!(PoolWaterEditorPlugin, Editor);
