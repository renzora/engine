//! Built-in inspector entries for core Bevy components.
//!
//! These live in the editor framework so that they are always available,
//! regardless of which engine plugins are loaded.

use bevy::light::{
    EnvironmentMapLight, LightProbe, VolumetricLight,
};
use bevy::pbr::Lightmap;
use bevy::prelude::*;

use crate::inspector_registry::{
    FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry,
};
use crate::spawn_registry::{ComponentIconEntry, ComponentIconRegistry};
use crate::{EntityLabelColor, EntityTag};

/// Register spawn presets for core Bevy entity types.
pub fn register_bevy_presets(registry: &mut crate::SpawnRegistry) {
    use crate::spawn_registry::EntityPreset;

    registry.register(EntityPreset {
        id: "empty_entity",
        display_name: "Empty Entity",
        icon: "circle",
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
        icon: "sun",
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
        icon: "lightbulb",
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
        icon: "flashlight",
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
        icon: "sun-dim",
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
        icon: "video-camera",
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
                world
                    .entity_mut(entity)
                    .insert(renzora::core::DefaultCamera);
            }
            entity
        },
    });

    // ── 2D presets ──────────────────────────────────────────────────────────
    registry.register(EntityPreset {
        id: "camera_2d",
        display_name: "Camera 2D",
        icon: "video-camera",
        category: "camera",
        spawn_fn: |world| {
            // Mirror the 3D preset: tag with SceneCamera so play mode can
            // discover it, and DefaultCamera if this is the first scene
            // camera in the world. Without these, the Play button stays
            // disabled and the play-mode picker can't find the camera.
            let mut count = 0u32;
            let mut q = world.query_filtered::<(), With<renzora::SceneCamera>>();
            for _ in q.iter(world) {
                count += 1;
            }
            let is_first = count == 0;
            let name = if is_first {
                "Camera 2D".to_string()
            } else {
                format!("Camera 2D ({})", count + 1)
            };
            // Camera 2D spawns at world origin. The Godot-style
            // alignment (world (0, 0) renders at the top-left of the
            // viewport, matching the window-area outline) is handled
            // by an observer that sets `viewport_origin = (0, 1)` on
            // every Camera2d insert — see `renzora_engine::camera`.
            let entity = world
                .spawn((
                    Name::new(name),
                    Transform::default(),
                    Camera2d,
                    Camera {
                        is_active: false,
                        ..default()
                    },
                    renzora::SceneCamera,
                ))
                .id();
            if is_first {
                world
                    .entity_mut(entity)
                    .insert(renzora::core::DefaultCamera);
            }
            entity
        },
    });

    registry.register(EntityPreset {
        id: "sprite",
        display_name: "Sprite",
        icon: "image-square",
        category: "nodes_2d",
        // Default spawn is a 100×100 light-blue square. Bevy's Sprite renders
        // a flat-colored quad when no image is set — gives the user something
        // visible to drag around in the 2D view before they assign a texture.
        // SpriteImagePath ships empty so the inspector shows a drop slot the
        // user can drag a texture onto; the rehydration system reads that
        // path back into Sprite.image.
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Sprite"),
                    Transform::default(),
                    Sprite {
                        color: Color::srgba(0.5, 0.7, 1.0, 1.0),
                        custom_size: Some(Vec2::new(100.0, 100.0)),
                        ..default()
                    },
                    renzora::core::SpriteImagePath::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "node_2d",
        display_name: "Node 2D",
        icon: "shapes",
        category: "nodes_2d",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Node 2D"),
                    Transform::default(),
                    renzora::core::Node2d,
                ))
                .id()
        },
    });
}

/// Register hierarchy icons for core Bevy components.
pub fn register_bevy_icons(registry: &mut ComponentIconRegistry) {
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<Camera3d>(),
        name: "Camera",
        icon: "video-camera",
        color: [100, 180, 255],
        priority: 100,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<Camera2d>(),
        name: "Camera 2D",
        icon: "video-camera",
        color: [180, 220, 130],
        priority: 100,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<Sprite>(),
        name: "Sprite",
        icon: "image-square",
        color: [180, 220, 130],
        priority: 50,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<renzora::core::Node2d>(),
        name: "Node 2D",
        icon: "shapes",
        color: [180, 220, 130],
        priority: 70,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<DirectionalLight>(),
        name: "Directional Light",
        icon: "sun",
        color: [255, 220, 100],
        priority: 90,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<PointLight>(),
        name: "Point Light",
        icon: "lightbulb",
        color: [255, 200, 80],
        priority: 90,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<SpotLight>(),
        name: "Spot Light",
        icon: "flashlight",
        color: [255, 200, 80],
        priority: 90,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<AmbientLight>(),
        name: "Ambient Light",
        icon: "sun-dim",
        color: [200, 200, 150],
        priority: 80,
        dynamic_icon_fn: None,
    });
    registry.register(ComponentIconEntry {
        type_id: std::any::TypeId::of::<Mesh3d>(),
        name: "Mesh",
        icon: "cube",
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
    registry.register(lightmap_entry());
    registry.register(volumetric_light_entry());
    // `volumetric_fog_entry()` removed -- `renzora_volumetric_fog` now
    // provides the inspector entry (same `type_id: "volumetric_fog"`)
    // backed by a Settings wrapper that routes from the
    // WorldEnvironment entity to all active cameras via EffectRouting.
    registry.register(sprite_image_entry());
}

fn sprite_image_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "sprite_image",
        display_name: "Sprite Image",
        icon: "image-square",
        category: "rendering",
        // Show for any Sprite — `SpriteImagePath` is auto-added on first
        // assignment if missing, so the user always sees the drop slot.
        has_fn: |world, entity| world.get::<Sprite>(entity).is_some(),
        add_fn: None,
        // Removing the path doesn't remove the Sprite, just clears the
        // image — so drop the path component back to default rather than
        // pulling it off entirely.
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<renzora::core::SpriteImagePath>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Image",
            field_type: FieldType::Asset {
                extensions: vec![
                    "png".into(),
                    "jpg".into(),
                    "jpeg".into(),
                    "webp".into(),
                    "ktx2".into(),
                    "rmip".into(),
                ],
            },
            get_fn: |world, entity| {
                let path = world
                    .get::<renzora::core::SpriteImagePath>(entity)
                    .map(|p| {
                        if p.0.is_empty() {
                            None
                        } else {
                            Some(p.0.clone())
                        }
                    })
                    .unwrap_or(None);
                Some(FieldValue::Asset(path))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Asset(path) = val {
                    let path_str = path.unwrap_or_default();
                    // Always `insert` (replace) so the lifecycle observer
                    // fires; in-place `Mut<>` mutation doesn't trigger
                    // observers, which is the path that's actually wired
                    // up to bind the texture handle.
                    world
                        .entity_mut(entity)
                        .insert(renzora::core::SpriteImagePath(path_str));
                }
            },
        }],
    }
}

fn name_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "name",
        display_name: "Name",
        icon: "tag",
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
                        let color = [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8];
                        world.entity_mut(entity).insert(EntityLabelColor(color));
                    }
                },
            },
        ],
    }
}

fn transform_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "transform",
        display_name: "Transform",
        icon: "arrows-out-cardinal",
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
    }
}

fn visibility_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "visibility",
        display_name: "Visibility",
        icon: "eye",
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
    }
}

fn directional_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "directional_light",
        display_name: "Directional Light",
        icon: "sun",
        category: "lighting",
        has_fn: |world, entity| world.get::<DirectionalLight>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(DirectionalLight::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<DirectionalLight>();
        }),
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
    }
}

fn point_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "point_light",
        display_name: "Point Light",
        icon: "lightbulb",
        category: "lighting",
        has_fn: |world, entity| world.get::<PointLight>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(PointLight::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<PointLight>();
        }),
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
    }
}

fn spot_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "spot_light",
        display_name: "Spot Light",
        icon: "flashlight",
        category: "lighting",
        has_fn: |world, entity| world.get::<SpotLight>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SpotLight::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<SpotLight>();
        }),
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
    }
}

fn ambient_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ambient_light",
        display_name: "Ambient Light",
        icon: "sun-dim",
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
    }
}

fn camera_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "camera",
        display_name: "Camera",
        icon: "video-camera",
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
                    world
                        .get::<Camera>(entity)
                        .map(|c| FieldValue::ReadOnly(format!("{:?}", c.output_mode)))
                },
                set_fn: |_, _, _| {},
            },
            FieldDef {
                name: "FOV",
                field_type: FieldType::Float {
                    speed: 0.5,
                    min: 10.0,
                    max: 170.0,
                },
                get_fn: |world, entity| {
                    world.get::<Projection>(entity).and_then(|p| match p {
                        Projection::Perspective(persp) => {
                            Some(FieldValue::Float(persp.fov.to_degrees()))
                        }
                        _ => None,
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut p) = world.get_mut::<Projection>(entity) {
                            if let Projection::Perspective(ref mut persp) = *p {
                                persp.fov = v.clamp(10.0, 170.0).to_radians();
                            }
                        }
                    }
                },
            },
        ],
    }
}

fn camera3d_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "camera3d",
        display_name: "Camera 3D",
        icon: "aperture",
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
    }
}

fn mesh3d_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "mesh3d",
        display_name: "Mesh 3D",
        icon: "cube",
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
    }
}

fn environment_map_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "environment_map_light",
        display_name: "Environment Map",
        icon: "globe-hemisphere-east",
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
    }
}

fn light_probe_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "light_probe",
        display_name: "Light Probe",
        icon: "broadcast",
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
                world.get::<LightProbe>(entity).map(|_| {
                    FieldValue::ReadOnly(
                        "Marker — add an EnvironmentMapLight".to_string(),
                    )
                })
            },
            set_fn: |_, _, _| {},
        }],
    }
}

fn lightmap_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "lightmap",
        display_name: "Lightmap",
        icon: "image",
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
                        FieldValue::ReadOnly(format!(
                            "({:.2}, {:.2})",
                            l.uv_rect.min.x, l.uv_rect.min.y
                        ))
                    })
                },
                set_fn: |_, _, _| {},
            },
            FieldDef {
                name: "UV Max",
                field_type: FieldType::ReadOnly,
                get_fn: |world, entity| {
                    world.get::<Lightmap>(entity).map(|l| {
                        FieldValue::ReadOnly(format!(
                            "({:.2}, {:.2})",
                            l.uv_rect.max.x, l.uv_rect.max.y
                        ))
                    })
                },
                set_fn: |_, _, _| {},
            },
        ],
    }
}

fn volumetric_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "volumetric_light",
        display_name: "Volumetric Light",
        icon: "sun-horizon",
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
                world.get::<VolumetricLight>(entity).map(|_| {
                    FieldValue::ReadOnly(
                        "Enables god rays (requires VolumetricFog on camera)".to_string(),
                    )
                })
            },
            set_fn: |_, _, _| {},
        }],
    }
}

// `volumetric_fog_entry` removed -- now provided by
// `renzora_volumetric_fog::inspector_entry`, which routes settings
// from a WorldEnvironment-style source onto every active camera via
// `EffectRouting`. Keeping the direct-on-camera variant here would
// create a duplicate inspector entry with the same `type_id`.
