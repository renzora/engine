//! Built-in inspector entries for core Bevy components.
//!
//! These live in the editor framework so that they are always available,
//! regardless of which engine plugins are loaded.

use bevy::prelude::*;
use bevy::pbr::Lightmap;
use bevy::light::{EnvironmentMapLight, IrradianceVolume, LightProbe, VolumetricFog, VolumetricLight};
use egui_phosphor::regular;

use crate::inspector_registry::{FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry};
use crate::spawn_registry::{ComponentIconEntry, ComponentIconRegistry};
use crate::{EntityLabelColor, EntityTag};

/// Register spawn presets for core Bevy entity types.
pub fn register_bevy_presets(registry: &mut crate::SpawnRegistry) {
    use crate::spawn_registry::EntityPreset;

    registry.register(EntityPreset {
        id: "empty_entity",
        display_name: "Empty Entity",
        icon: regular::CIRCLE,
        category: "general",
        spawn_fn: |world| {
            world
                .spawn((Name::new("Empty Entity"), Transform::default()))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "directional_light",
        display_name: "Directional Light",
        icon: regular::SUN,
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Directional Light"),
                    Transform::default(),
                    DirectionalLight::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "point_light",
        display_name: "Point Light",
        icon: regular::LIGHTBULB,
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Point Light"),
                    Transform::default(),
                    PointLight::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "spot_light",
        display_name: "Spot Light",
        icon: regular::FLASHLIGHT,
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Spot Light"),
                    Transform::default(),
                    SpotLight::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "ambient_light",
        display_name: "Ambient Light",
        icon: regular::SUN_DIM,
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Ambient Light"),
                    AmbientLight {
                        color: Color::WHITE,
                        brightness: 300.0,
                        ..default()
                    },
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "camera_3d",
        display_name: "Camera 3D",
        icon: regular::VIDEO_CAMERA,
        category: "camera",
        spawn_fn: |world| {
            let mut count = 0u32;
            let mut q = world.query_filtered::<(), With<renzora::SceneCamera>>();
            for _ in q.iter(world) {
                count += 1;
            }
            let is_first = count == 0;
            let name = if is_first {
                "Camera 3D".to_string()
            } else {
                format!("Camera 3D ({})", count + 1)
            };
            let entity = world
                .spawn((
                    Name::new(name),
                    Transform::default(),
                    Camera3d::default(),
                    Camera {
                        is_active: false,
                        ..default()
                    },
                    renzora::SceneCamera,
                ))
                .id();
            if is_first {
                world.entity_mut(entity).insert(renzora::core::DefaultCamera);
            }
            entity
        },
    });
}

/// Register hierarchy icons for core Bevy components.
pub fn register_bevy_icons(registry: &mut ComponentIconRegistry) {
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<Camera3d>(),
        icon: regular::VIDEO_CAMERA,
        color: [100, 180, 255],
        priority: 100,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<DirectionalLight>(),
        icon: regular::SUN,
        color: [255, 220, 100],
        priority: 90,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<PointLight>(),
        icon: regular::LIGHTBULB,
        color: [255, 200, 80],
        priority: 90,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<SpotLight>(),
        icon: regular::FLASHLIGHT,
        color: [255, 200, 80],
        priority: 90,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<AmbientLight>(),
        icon: regular::SUN_DIM,
        color: [200, 200, 150],
        priority: 80,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<Mesh3d>(),
        icon: regular::CUBE,
        color: [255, 170, 100],
        priority: 50,
        dynamic_icon_fn: None,
    });
}

/// Register inspector entries for core Bevy components (Name, Transform,
/// lights, cameras, etc.).  Called automatically by the editor framework
/// plugin — downstream crates do **not** need to call this.
pub fn register_bevy_inspectors(registry: &mut InspectorRegistry) {
    registry.register(name_entry());
    registry.register(transform_entry());
    registry.register(visibility_entry());
    registry.register(directional_light_entry());
    registry.register(point_light_entry());
    registry.register(spot_light_entry());
    registry.register(ambient_light_entry());
    registry.register(camera_entry());
    registry.register(camera3d_entry());
    registry.register(mesh3d_entry());
    registry.register(environment_map_light_entry());
    registry.register(light_probe_entry());
    registry.register(irradiance_volume_entry());
    registry.register(lightmap_entry());
    registry.register(volumetric_light_entry());
    registry.register(volumetric_fog_entry());
}

fn name_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "name",
        display_name: "Name",
        icon: regular::TAG,
        category: "transform",
        has_fn: |world, entity| world.get::<Name>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Name",
                field_type: FieldType::String,
                get_fn: |world, entity| {
                    world
                        .get::<Name>(entity)
                        .map(|n| FieldValue::String(n.as_str().to_string()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::String(v) = val {
                        if let Some(mut n) = world.get_mut::<Name>(entity) {
                            *n = Name::new(v);
                        }
                    }
                },
            },
            FieldDef {
                name: "Tag",
                field_type: FieldType::String,
                get_fn: |world, entity| {
                    Some(FieldValue::String(
                        world
                            .get::<EntityTag>(entity)
                            .map(|t| t.tag.clone())
                            .unwrap_or_default(),
                    ))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::String(v) = val {
                        if let Some(mut t) = world.get_mut::<EntityTag>(entity) {
                            t.tag = v;
                        } else {
                            world.entity_mut(entity).insert(EntityTag { tag: v });
                        }
                    }
                },
            },
            FieldDef {
                name: "Label Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    let c = world
                        .get::<EntityLabelColor>(entity)
                        .map(|lc| lc.0)
                        .unwrap_or([220, 222, 228]);
                    Some(FieldValue::Color([
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0,
                    ]))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        let color = [
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (b * 255.0) as u8,
                        ];
                        world.entity_mut(entity).insert(EntityLabelColor(color));
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn transform_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "transform",
        display_name: "Transform",
        icon: regular::ARROWS_OUT_CARDINAL,
        category: "transform",
        has_fn: |world, entity| world.get::<Transform>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
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
        custom_ui_fn: None,
    }
}

fn visibility_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "visibility",
        display_name: "Visibility",
        icon: regular::EYE,
        category: "rendering",
        has_fn: |world, entity| world.get::<Visibility>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Visible",
            field_type: FieldType::Bool,
            get_fn: |world, entity| {
                world.get::<Visibility>(entity).map(|v| {
                    FieldValue::Bool(matches!(*v, Visibility::Inherited | Visibility::Visible))
                })
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Bool(v) = val {
                    if let Some(mut vis) = world.get_mut::<Visibility>(entity) {
                        *vis = if v {
                            Visibility::Inherited
                        } else {
                            Visibility::Hidden
                        };
                    }
                }
            },
        }],
        custom_ui_fn: None,
    }
}

fn directional_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "directional_light",
        display_name: "Directional Light",
        icon: regular::SUN,
        category: "lighting",
        has_fn: |world, entity| world.get::<DirectionalLight>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
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
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<DirectionalLight>(entity).map(|l| {
                        let c = l.color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut l) = world.get_mut::<DirectionalLight>(entity) {
                            l.color = Color::srgb(r, g, b);
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
        custom_ui_fn: None,
    }
}

fn point_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "point_light",
        display_name: "Point Light",
        icon: regular::LIGHTBULB,
        category: "lighting",
        has_fn: |world, entity| world.get::<PointLight>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<PointLight>(entity).map(|l| {
                        let c = l.color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut l) = world.get_mut::<PointLight>(entity) {
                            l.color = Color::srgb(r, g, b);
                        }
                    }
                },
            },
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
        custom_ui_fn: None,
    }
}

fn spot_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "spot_light",
        display_name: "Spot Light",
        icon: regular::FLASHLIGHT,
        category: "lighting",
        has_fn: |world, entity| world.get::<SpotLight>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<SpotLight>(entity).map(|l| {
                        let c = l.color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.color = Color::srgb(r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 100.0,
                    min: 0.0,
                    max: 10_000_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Float(l.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
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
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Float(l.range))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.range = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Inner Angle",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.57,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Float(l.inner_angle))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.inner_angle = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Outer Angle",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.57,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Float(l.outer_angle))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.outer_angle = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Shadows",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<SpotLight>(entity)
                        .map(|l| FieldValue::Bool(l.shadows_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.shadows_enabled = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn ambient_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ambient_light",
        display_name: "Ambient Light",
        icon: regular::SUN_DIM,
        category: "lighting",
        has_fn: |world, entity| world.get::<AmbientLight>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<AmbientLight>(entity).map(|l| {
                        let c = l.color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut l) = world.get_mut::<AmbientLight>(entity) {
                            l.color = Color::srgb(r, g, b);
                        }
                    }
                },
            },
            FieldDef {
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
            },
        ],
        custom_ui_fn: None,
    }
}

fn camera_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "camera",
        display_name: "Camera",
        icon: regular::VIDEO_CAMERA,
        category: "camera",
        has_fn: |world, entity| world.get::<Camera>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Order",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: -100.0,
                    max: 100.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Camera>(entity)
                        .map(|c| FieldValue::Float(c.order as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut c) = world.get_mut::<Camera>(entity) {
                            c.order = v as isize;
                        }
                    }
                },
            },
            FieldDef {
                name: "Active",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<Camera>(entity)
                        .map(|c| FieldValue::Bool(c.is_active))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut c) = world.get_mut::<Camera>(entity) {
                            c.is_active = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Output",
                field_type: FieldType::ReadOnly,
                get_fn: |world, entity| {
                    world.get::<Camera>(entity).map(|c| {
                        FieldValue::ReadOnly(format!("{:?}", c.output_mode))
                    })
                },
                set_fn: |_, _, _| {},
            },
        ],
        custom_ui_fn: None,
    }
}

fn camera3d_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "camera3d",
        display_name: "Camera 3D",
        icon: regular::APERTURE,
        category: "camera",
        has_fn: |world, entity| world.get::<Camera3d>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Type",
            field_type: FieldType::ReadOnly,
            get_fn: |world, entity| {
                world
                    .get::<Camera3d>(entity)
                    .map(|_| FieldValue::ReadOnly("Perspective 3D".to_string()))
            },
            set_fn: |_, _, _| {},
        }],
        custom_ui_fn: None,
    }
}

fn mesh3d_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "mesh3d",
        display_name: "Mesh 3D",
        icon: regular::CUBE,
        category: "rendering",
        has_fn: |world, entity| world.get::<Mesh3d>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
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
        custom_ui_fn: None,
    }
}

fn environment_map_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "environment_map_light",
        display_name: "Environment Map",
        icon: regular::GLOBE_HEMISPHERE_EAST,
        category: "lighting",
        has_fn: |world, entity| world.get::<EnvironmentMapLight>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(EnvironmentMapLight {
                diffuse_map: Handle::default(),
                specular_map: Handle::default(),
                intensity: 1.0,
                rotation: Quat::IDENTITY,
                affects_lightmapped_mesh_diffuse: true,
            });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<EnvironmentMapLight>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 100.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<EnvironmentMapLight>(entity)
                        .map(|l| FieldValue::Float(l.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<EnvironmentMapLight>(entity) {
                            l.intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Affects Lightmapped",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<EnvironmentMapLight>(entity)
                        .map(|l| FieldValue::Bool(l.affects_lightmapped_mesh_diffuse))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<EnvironmentMapLight>(entity) {
                            l.affects_lightmapped_mesh_diffuse = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Rotation",
                field_type: FieldType::Vec3 { speed: 0.5 },
                get_fn: |world, entity| {
                    world.get::<EnvironmentMapLight>(entity).map(|l| {
                        let (x, y, z) = l.rotation.to_euler(EulerRot::XYZ);
                        FieldValue::Vec3([x.to_degrees(), y.to_degrees(), z.to_degrees()])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        if let Some(mut l) = world.get_mut::<EnvironmentMapLight>(entity) {
                            l.rotation = Quat::from_euler(
                                EulerRot::XYZ,
                                v[0].to_radians(),
                                v[1].to_radians(),
                                v[2].to_radians(),
                            );
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn light_probe_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "light_probe",
        display_name: "Light Probe",
        icon: regular::BROADCAST,
        category: "lighting",
        has_fn: |world, entity| world.get::<LightProbe>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(LightProbe);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<LightProbe>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Info",
            field_type: FieldType::ReadOnly,
            get_fn: |world, entity| {
                world
                    .get::<LightProbe>(entity)
                    .map(|_| FieldValue::ReadOnly("Marker — add EnvironmentMapLight or IrradianceVolume".to_string()))
            },
            set_fn: |_, _, _| {},
        }],
        custom_ui_fn: None,
    }
}

fn irradiance_volume_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "irradiance_volume",
        display_name: "Irradiance Volume",
        icon: regular::CUBE_TRANSPARENT,
        category: "lighting",
        has_fn: |world, entity| world.get::<IrradianceVolume>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(IrradianceVolume {
                voxels: Handle::default(),
                intensity: 1.0,
                affects_lightmapped_meshes: true,
            });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<IrradianceVolume>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 100.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<IrradianceVolume>(entity)
                        .map(|v| FieldValue::Float(v.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(f) = val {
                        if let Some(mut v) = world.get_mut::<IrradianceVolume>(entity) {
                            v.intensity = f;
                        }
                    }
                },
            },
            FieldDef {
                name: "Affects Lightmapped",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<IrradianceVolume>(entity)
                        .map(|v| FieldValue::Bool(v.affects_lightmapped_meshes))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if let Some(mut v) = world.get_mut::<IrradianceVolume>(entity) {
                            v.affects_lightmapped_meshes = b;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn lightmap_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "lightmap",
        display_name: "Lightmap",
        icon: regular::IMAGE,
        category: "lighting",
        has_fn: |world, entity| world.get::<Lightmap>(entity).is_some(),
        add_fn: None,
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<Lightmap>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "UV Min",
                field_type: FieldType::ReadOnly,
                get_fn: |world, entity| {
                    world.get::<Lightmap>(entity).map(|l| {
                        FieldValue::ReadOnly(format!("({:.2}, {:.2})", l.uv_rect.min.x, l.uv_rect.min.y))
                    })
                },
                set_fn: |_, _, _| {},
            },
            FieldDef {
                name: "UV Max",
                field_type: FieldType::ReadOnly,
                get_fn: |world, entity| {
                    world.get::<Lightmap>(entity).map(|l| {
                        FieldValue::ReadOnly(format!("({:.2}, {:.2})", l.uv_rect.max.x, l.uv_rect.max.y))
                    })
                },
                set_fn: |_, _, _| {},
            },
        ],
        custom_ui_fn: None,
    }
}

fn volumetric_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "volumetric_light",
        display_name: "Volumetric Light",
        icon: regular::SUN_HORIZON,
        category: "lighting",
        has_fn: |world, entity| world.get::<VolumetricLight>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(VolumetricLight);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<VolumetricLight>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Info",
            field_type: FieldType::ReadOnly,
            get_fn: |world, entity| {
                world
                    .get::<VolumetricLight>(entity)
                    .map(|_| FieldValue::ReadOnly("Enables god rays (requires VolumetricFog on camera)".to_string()))
            },
            set_fn: |_, _, _| {},
        }],
        custom_ui_fn: None,
    }
}

fn volumetric_fog_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "volumetric_fog",
        display_name: "Volumetric Fog",
        icon: regular::CLOUD_FOG,
        category: "lighting",
        has_fn: |world, entity| world.get::<VolumetricFog>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(VolumetricFog::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<VolumetricFog>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Ambient Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<VolumetricFog>(entity).map(|f| {
                        let c = f.ambient_color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut f) = world.get_mut::<VolumetricFog>(entity) {
                            f.ambient_color = Color::srgb(r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Ambient Intensity",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<VolumetricFog>(entity)
                        .map(|f| FieldValue::Float(f.ambient_intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut f) = world.get_mut::<VolumetricFog>(entity) {
                            f.ambient_intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Step Count",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 1.0,
                    max: 256.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<VolumetricFog>(entity)
                        .map(|f| FieldValue::Float(f.step_count as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut f) = world.get_mut::<VolumetricFog>(entity) {
                            f.step_count = v as u32;
                        }
                    }
                },
            },
            FieldDef {
                name: "Jitter",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<VolumetricFog>(entity)
                        .map(|f| FieldValue::Float(f.jitter))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut f) = world.get_mut::<VolumetricFog>(entity) {
                            f.jitter = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}
