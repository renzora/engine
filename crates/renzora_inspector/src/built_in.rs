//! Built-in inspector entries for core Bevy components.

use bevy::prelude::*;
use egui_phosphor::regular;
use renzora_editor::{FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry};

/// Register inspector entries for built-in Bevy components.
pub fn register_built_in_inspectors(registry: &mut InspectorRegistry) {
    registry.register(transform_entry());
    registry.register(directional_light_entry());
    registry.register(point_light_entry());
    registry.register(ambient_light_entry());
    registry.register(camera3d_entry());
    registry.register(mesh3d_entry());
}

fn transform_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "transform",
        display_name: "Transform",
        icon: regular::ARROWS_OUT_CARDINAL,
        category: "transform",
        has_fn: |world, entity| world.get::<Transform>(entity).is_some(),
        fields: vec![
            FieldDef {
                name: "Position",
                field_type: FieldType::Vec3 { speed: 0.1 },
                get_fn: |world, entity| {
                    world
                        .get::<Transform>(entity)
                        .map(|t| FieldValue::Vec3(t.translation.into()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        if let Some(mut t) = world.get_mut::<Transform>(entity) {
                            t.translation = Vec3::from(v);
                        }
                    }
                },
            },
            FieldDef {
                name: "Rotation",
                field_type: FieldType::Vec3 { speed: 0.5 },
                get_fn: |world, entity| {
                    world.get::<Transform>(entity).map(|t| {
                        let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                        FieldValue::Vec3([x.to_degrees(), y.to_degrees(), z.to_degrees()])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        if let Some(mut t) = world.get_mut::<Transform>(entity) {
                            t.rotation = Quat::from_euler(
                                EulerRot::XYZ,
                                v[0].to_radians(),
                                v[1].to_radians(),
                                v[2].to_radians(),
                            );
                        }
                    }
                },
            },
            FieldDef {
                name: "Scale",
                field_type: FieldType::Vec3 { speed: 0.01 },
                get_fn: |world, entity| {
                    world
                        .get::<Transform>(entity)
                        .map(|t| FieldValue::Vec3(t.scale.into()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        if let Some(mut t) = world.get_mut::<Transform>(entity) {
                            t.scale = Vec3::from(v);
                        }
                    }
                },
            },
        ],
    }
}

fn directional_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "directional_light",
        display_name: "Directional Light",
        icon: regular::SUN,
        category: "lighting",
        has_fn: |world, entity| world.get::<DirectionalLight>(entity).is_some(),
        fields: vec![
            FieldDef {
                name: "Illuminance",
                field_type: FieldType::Float {
                    speed: 100.0,
                    min: 0.0,
                    max: 200_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<DirectionalLight>(entity)
                        .map(|l| FieldValue::Float(l.illuminance))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<DirectionalLight>(entity) {
                            l.illuminance = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Shadows",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<DirectionalLight>(entity)
                        .map(|l| FieldValue::Bool(l.shadows_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<DirectionalLight>(entity) {
                            l.shadows_enabled = v;
                        }
                    }
                },
            },
        ],
    }
}

fn point_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "point_light",
        display_name: "Point Light",
        icon: regular::LIGHTBULB,
        category: "lighting",
        has_fn: |world, entity| world.get::<PointLight>(entity).is_some(),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 100.0,
                    min: 0.0,
                    max: 10_000_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PointLight>(entity)
                        .map(|l| FieldValue::Float(l.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<PointLight>(entity) {
                            l.intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Range",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 1000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<PointLight>(entity)
                        .map(|l| FieldValue::Float(l.range))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<PointLight>(entity) {
                            l.range = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Shadows",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<PointLight>(entity)
                        .map(|l| FieldValue::Bool(l.shadows_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<PointLight>(entity) {
                            l.shadows_enabled = v;
                        }
                    }
                },
            },
        ],
    }
}

fn ambient_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ambient_light",
        display_name: "Ambient Light",
        icon: regular::SUN_DIM,
        category: "lighting",
        has_fn: |world, entity| world.get::<AmbientLight>(entity).is_some(),
        fields: vec![FieldDef {
            name: "Brightness",
            field_type: FieldType::Float {
                speed: 10.0,
                min: 0.0,
                max: 100_000.0,
            },
            get_fn: |world, entity| {
                world
                    .get::<AmbientLight>(entity)
                    .map(|l| FieldValue::Float(l.brightness))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Float(v) = val {
                    if let Some(mut l) = world.get_mut::<AmbientLight>(entity) {
                        l.brightness = v;
                    }
                }
            },
        }],
    }
}

fn camera3d_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "camera3d",
        display_name: "Camera 3D",
        icon: regular::VIDEO_CAMERA,
        category: "camera",
        has_fn: |world, entity| world.get::<Camera3d>(entity).is_some(),
        fields: vec![FieldDef {
            name: "Status",
            field_type: FieldType::ReadOnly,
            get_fn: |world, entity| {
                world
                    .get::<Camera3d>(entity)
                    .map(|_| FieldValue::ReadOnly("Active".to_string()))
            },
            set_fn: |_, _, _| {},
        }],
    }
}

fn mesh3d_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "mesh3d",
        display_name: "Mesh 3D",
        icon: regular::CUBE,
        category: "rendering",
        has_fn: |world, entity| world.get::<Mesh3d>(entity).is_some(),
        fields: vec![FieldDef {
            name: "Mesh",
            field_type: FieldType::ReadOnly,
            get_fn: |world, entity| {
                world
                    .get::<Mesh3d>(entity)
                    .map(|_| FieldValue::ReadOnly("Mesh attached".to_string()))
            },
            set_fn: |_, _, _| {},
        }],
    }
}
