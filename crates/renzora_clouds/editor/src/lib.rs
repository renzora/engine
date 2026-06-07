//! Editor-only half of `renzora_clouds` — the Clouds inspector.
//!
//! `renzora_clouds` compiles lean (no `editor` feature, no egui-phosphor). This
//! crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(CloudsEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::prelude::*;
use renzora::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_clouds::CloudsData;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "clouds",
        display_name: "Clouds",
        icon: "cloud-sun",
        category: "rendering",
        has_fn: |world, entity| world.get::<CloudsData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CloudsData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<CloudsData>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<CloudsData>(entity)
                .map(|d| d.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                d.enabled = val;
            }
        }),
        fields: vec![
            FieldDef {
                name: "Coverage",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.coverage))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.coverage = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Density",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.density))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.density = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Scale",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.1,
                    max: 50.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.scale))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.scale = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Speed",
                field_type: FieldType::Float {
                    speed: 0.001,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.speed = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Wind Direction",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 0.0,
                    max: 360.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.wind_direction))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.wind_direction = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Altitude",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.altitude))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.altitude = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Color([d.color.0, d.color.1, d.color.2]))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.color = (r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Shadow Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<CloudsData>(entity).map(|d| {
                        FieldValue::Color([d.shadow_color.0, d.shadow_color.1, d.shadow_color.2])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.shadow_color = (r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Absorption",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 5.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.absorption))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.absorption = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Silver Lining",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 2.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.silver_intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.silver_intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Silver Spread",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.01,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.silver_spread))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.silver_spread = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Powder Effect",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.powder_strength))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.powder_strength = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Ambient",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.ambient_brightness))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.ambient_brightness = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Horizon Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<CloudsData>(entity).map(|d| {
                        FieldValue::Color([d.horizon_color.0, d.horizon_color.1, d.horizon_color.2])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.horizon_color = (r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Atmosphere",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.atmosphere_strength))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.atmosphere_strength = v;
                        }
                    }
                },
            },
        ],
    }
}

/// Editor-scope companion to `renzora_clouds::CloudsPlugin`.
#[derive(Default)]
pub struct CloudsEditorPlugin;

impl Plugin for CloudsEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] CloudsEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(CloudsEditorPlugin, Editor);
