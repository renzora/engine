//! The Pool Water inspector.
//!
//! In the workspace this was a **separate crate**, `renzora_pool_water_editor`,
//! because the inspector's dependency on `renzora_editor_framework` was not
//! optional and would otherwise have compiled the whole editor framework into
//! every shipped game — and a cargo feature could not have separated them, since
//! features unify across a `--workspace` build.
//!
//! Neither problem exists here. `renzora_editor_framework` only *re-exports*
//! `AppEditorExt` / `InspectorEntry` / `FieldDef` from the contract crate, so
//! this imports them from `renzora` directly and links no framework at all. The
//! two halves are one plugin again, and a game that loads it pays a single `Vec`
//! push into a registry it never reads.

use bevy::prelude::*;
use renzora::AppEditorExt;

use crate::PoolWater;

pub(crate) fn pool_water_inspector_entry() -> renzora::InspectorEntry {
    use renzora::{FieldDef, FieldType, FieldValue, InspectorEntry};

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

/// Registered from the one plugin's `build`, unconditionally — see the note in
/// `lib.rs` on why there is no `cfg(feature = "editor")` here.
pub(crate) fn register(app: &mut App) {
    app.register_inspector(pool_water_inspector_entry());
}
