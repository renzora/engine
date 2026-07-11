//! Built-in inspector entries for core Bevy components.
//!
//! These live in the editor framework so that they are always available,
//! regardless of which engine plugins are loaded.

use bevy::light::{
    EnvironmentMapLight, GeneratedEnvironmentMapLight, LightProbe, ParallaxCorrection,
    VolumetricLight,
};
use bevy::pbr::Lightmap;
use bevy::prelude::*;

use crate::{
    FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry,
};
use crate::{ComponentIconEntry, ComponentIconRegistry};
use crate::{EntityLabelColor, EntityTag};

/// Register spawn presets for core Bevy entity types.
pub fn register_bevy_presets(registry: &mut crate::SpawnRegistry) {
    use crate::EntityPreset;

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

    // Bevy 0.19 rectangular area light. Edited via `rect_light_entry()`
    // (color/intensity/range/width/height). `RectLight` requires
    // Transform/Visibility (auto-added). The rectangle lies in the local XY
    // plane and emits toward local -Z, so an identity transform would sit it at
    // the origin firing sideways (grazing the ground into a thin fan). Spawn it
    // elevated and aimed straight down so it casts a recognizable rectangle.
    registry.register(EntityPreset {
        id: "rect_light",
        display_name: "Rect Light",
        icon: "square",
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Rect Light"),
                    Transform::from_xyz(0.0, 3.0, 0.0)
                        .looking_at(Vec3::ZERO, Vec3::Z),
                    RectLight::default(),
                ))
                .id()
        },
    });

    // Bevy 0.19 parallax-corrected reflection probe: a `LightProbe` bounded by
    // the entity's Transform, with a `ReflectionProbeSource` (the authored
    // cubemap/HDR path). The GPU `GeneratedEnvironmentMapLight` is **not** added
    // here — `renzora_environment_map` attaches it only once a valid power-of-two
    // cube is ready, because bevy's filter would otherwise run on the 1×1 default
    // handle and spam GPU validation errors. `ParallaxCorrection::Auto` is set
    // explicitly so the box is editable from frame 0.
    registry.register(EntityPreset {
        id: "reflection_probe",
        display_name: "Reflection Probe",
        icon: "globe",
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Reflection Probe"),
                    Transform::from_scale(Vec3::splat(4.0)),
                    Visibility::default(),
                    LightProbe::default(),
                    ParallaxCorrection::Auto,
                    renzora::core::ReflectionProbeSource::default(),
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
        type_id: std::any::TypeId::of::<RectLight>(),
        name: "Rect Light",
        icon: "square",
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
    registry.register(rect_light_entry());
    registry.register(ambient_light_entry());
    registry.register(generated_environment_map_entry());
    registry.register(text_font_entry());
    registry.register(text_rich_entry());
    registry.register(camera_entry());
    registry.register(camera_presets_entry());
    registry.register(camera3d_entry());
    registry.register(mesh3d_entry());
    registry.register(environment_map_light_entry());
    registry.register(light_probe_entry());
    registry.register(parallax_correction_entry());
    registry.register(lightmap_entry());
    registry.register(volumetric_light_entry());
    // `volumetric_fog_entry()` removed -- `renzora_volumetric_fog` now
    // provides the inspector entry (same `type_id: "volumetric_fog"`)
    // backed by a Settings wrapper that routes from the
    // WorldEnvironment entity to all active cameras via EffectRouting.
    // The sheet grid (H/V Frames, Frame) is merged into the Sprite Image
    // section — there is no separate Sprite Sheet section. Same for Y Sort:
    // its toggle + fields live in the Sprite Image card.
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
        fields: vec![
            // The image control is a **dropdown** of the sheet names — never a
            // drop slot. (Two asset drop areas on one component fight over the
            // drop, which is why images went blank before.) Options are the
            // `SpriteImages` list, or the lone `SpriteImagePath` for a
            // single-image sprite. Value = selected index; as a dynamic dropdown
            // it's animatable, so it gets a keyframe button (`field_anim_path`
            // maps "Image" → `SpriteImages.index`).
            FieldDef {
                name: "Image",
                field_type: FieldType::DynamicEnum {
                    options: |world, entity| {
                        if let Some(imgs) = world.get::<renzora::core::SpriteImages>(entity) {
                            imgs.images
                                .iter()
                                .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string())
                                .collect()
                        } else if let Some(p) = world.get::<renzora::core::SpriteImagePath>(entity) {
                            if p.0.is_empty() {
                                Vec::new()
                            } else {
                                vec![p.0.rsplit(['/', '\\']).next().unwrap_or(&p.0).to_string()]
                            }
                        } else {
                            Vec::new()
                        }
                    },
                },
                get_fn: |world, entity| {
                    if let Some(imgs) = world.get::<renzora::core::SpriteImages>(entity) {
                        (!imgs.images.is_empty()).then_some(FieldValue::Float(imgs.index as f32))
                    } else if let Some(p) = world.get::<renzora::core::SpriteImagePath>(entity) {
                        (!p.0.is_empty()).then_some(FieldValue::Float(0.0))
                    } else {
                        None
                    }
                },
                set_fn: |world, entity, val| {
                    // Switching only applies to a multi-sheet list; a single
                    // image has one option and nothing to switch.
                    if let (FieldValue::Float(v), Some(mut s)) =
                        (val, world.get_mut::<renzora::core::SpriteImages>(entity))
                    {
                        s.index = v.round().max(0.0) as u32;
                    }
                },
            },
            // The one drop area: drop a sheet here to append it to the image
            // list (populating the Image dropdown). The first drop migrates a
            // single `SpriteImagePath` into a `SpriteImages` list.
            FieldDef {
                name: "Add Sheet",
                field_type: FieldType::Asset {
                    extensions: vec![
                        "png".into(), "jpg".into(), "jpeg".into(), "webp".into(), "ktx2".into(),
                        "rmip".into(),
                    ],
                },
                // Empty prompt — write-only ("drag to add").
                get_fn: |_world, _entity| Some(FieldValue::Asset(None)),
                set_fn: |world, entity, val| {
                    let FieldValue::Asset(Some(path)) = val else { return };
                    if path.is_empty() {
                        return;
                    }
                    if let Some(mut imgs) = world.get_mut::<renzora::core::SpriteImages>(entity) {
                        if !imgs.images.contains(&path) {
                            imgs.images.push(path.clone());
                        }
                        imgs.index = imgs.images.len().saturating_sub(1) as u32;
                    } else {
                        // Migrate the single SpriteImagePath into a list + append.
                        let mut images: Vec<String> = world
                            .get::<renzora::core::SpriteImagePath>(entity)
                            .map(|p| p.0.clone())
                            .filter(|s| !s.is_empty())
                            .into_iter()
                            .collect();
                        if !images.contains(&path) {
                            images.push(path);
                        }
                        let index = images.len().saturating_sub(1) as u32;
                        world
                            .entity_mut(entity)
                            .insert(renzora::core::SpriteImages { images, index });
                    }
                },
            },
            // Sheet grid — merged in from the old separate "Sprite Sheet"
            // section (Godot's Sprite2D carries these too). Backed by the
            // `SpriteSheet` component, which stays the data + the thing the
            // renderer/timeline/tilemap use; setting any of these creates it on
            // demand. `field_anim_path` maps their keyframes onto `SpriteSheet`.
            FieldDef {
                name: "H Frames",
                field_type: FieldType::Int { min: 1.0, max: 1024.0 },
                get_fn: |world, entity| {
                    Some(FieldValue::Float(
                        world.get::<renzora::core::SpriteSheet>(entity).map(|s| s.hframes).unwrap_or(1)
                            as f32,
                    ))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        let h = (v.round().max(1.0)) as u32;
                        if let Some(mut s) = world.get_mut::<renzora::core::SpriteSheet>(entity) {
                            s.hframes = h;
                        } else {
                            world.entity_mut(entity).insert(renzora::core::SpriteSheet {
                                hframes: h,
                                vframes: 1,
                                frame: 0,
                            });
                        }
                    }
                },
            },
            FieldDef {
                name: "V Frames",
                field_type: FieldType::Int { min: 1.0, max: 1024.0 },
                get_fn: |world, entity| {
                    Some(FieldValue::Float(
                        world.get::<renzora::core::SpriteSheet>(entity).map(|s| s.vframes).unwrap_or(1)
                            as f32,
                    ))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        let vf = (v.round().max(1.0)) as u32;
                        if let Some(mut s) = world.get_mut::<renzora::core::SpriteSheet>(entity) {
                            s.vframes = vf;
                        } else {
                            world.entity_mut(entity).insert(renzora::core::SpriteSheet {
                                hframes: 1,
                                vframes: vf,
                                frame: 0,
                            });
                        }
                    }
                },
            },
            FieldDef {
                name: "Frame",
                field_type: FieldType::Int { min: 0.0, max: 1_048_576.0 },
                get_fn: |world, entity| {
                    Some(FieldValue::Float(
                        world.get::<renzora::core::SpriteSheet>(entity).map(|s| s.frame).unwrap_or(0)
                            as f32,
                    ))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        let f = (v.round().max(0.0)) as u32;
                        if let Some(mut s) = world.get_mut::<renzora::core::SpriteSheet>(entity) {
                            s.frame = f;
                        } else {
                            world.entity_mut(entity).insert(renzora::core::SpriteSheet {
                                hframes: 1,
                                vframes: 1,
                                frame: f,
                            });
                        }
                    }
                },
            },
            // Mirror the sprite about its axes. These are Bevy's own
            // `Sprite::flip_x` / `flip_y` — a pure render-side flip, so unlike a
            // negative Transform scale they don't mirror children, colliders, or
            // gizmos. Written in place: flipping toggles no lifecycle observers.
            FieldDef {
                name: "Flip X",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world.get::<Sprite>(entity).map(|s| FieldValue::Bool(s.flip_x))
                },
                set_fn: |world, entity, val| {
                    if let (FieldValue::Bool(b), Some(mut s)) =
                        (val, world.get_mut::<Sprite>(entity))
                    {
                        s.flip_x = b;
                    }
                },
            },
            FieldDef {
                name: "Flip Y",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world.get::<Sprite>(entity).map(|s| FieldValue::Bool(s.flip_y))
                },
                set_fn: |world, entity, val| {
                    if let (FieldValue::Bool(b), Some(mut s)) =
                        (val, world.get_mut::<Sprite>(entity))
                    {
                        s.flip_y = b;
                    }
                },
            },
            // Y-sort — merged in like the sheet grid (Godot's Sprite2D keeps
            // its y-sort knob on the sprite too). The toggle adds/removes the
            // `YSort` component; the two fields below appear once it's on
            // (`get_fn` → None hides them). The engine's `apply_y_sort` system
            // owns the actual Z writes.
            FieldDef {
                name: "Y Sort",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    Some(FieldValue::Bool(
                        world.get::<renzora::core::YSort>(entity).is_some(),
                    ))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if b {
                            if world.get::<renzora::core::YSort>(entity).is_none() {
                                world
                                    .entity_mut(entity)
                                    .insert(renzora::core::YSort::default());
                            }
                        } else {
                            world.entity_mut(entity).remove::<renzora::core::YSort>();
                        }
                    }
                },
            },
            // Where the sort point sits relative to the sprite center. A tall
            // prop wants its trunk/feet, i.e. roughly -half_height.
            FieldDef {
                name: "Sort Offset",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: -100_000.0,
                    max: 100_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<renzora::core::YSort>(entity)
                        .map(|y| FieldValue::Float(y.offset))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut y) = world.get_mut::<renzora::core::YSort>(entity) {
                            y.offset = v;
                        }
                    }
                },
            },
            // The Z band the entity sorts within (±0.5 around it). Keeps
            // y-sorted props above a ground tilemap at z 0.
            FieldDef {
                name: "Z Base",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: -10_000.0,
                    max: 10_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<renzora::core::YSort>(entity)
                        .map(|y| FieldValue::Float(y.z_base))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut y) = world.get_mut::<renzora::core::YSort>(entity) {
                            y.z_base = v;
                        }
                    }
                },
            },
        ],
    }
}

// Sprite-sheet frame cropping (`renzora::core::SpriteSheet`): slice the bound
// texture into an hframes × vframes grid and show one cell. Merged into the
// Sprite Image section above; the `frame` field is the one users animate from
// the animation panel (it shows up in the Add Property picker via reflection).

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
                    let t = world.get::<Transform>(entity)?;
                    // Prefer the typed-Euler cache (so the field holds 0..360+ and
                    // the middle axis doesn't snap at 90°); fall back to deriving
                    // from the quaternion when something else moved the entity.
                    let cache = world.get::<renzora::EditorEulerCache>(entity);
                    let deg = renzora::rotation_euler_deg(t.rotation, cache, "transform");
                    Some(FieldValue::Vec3(deg.to_array()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        let q = renzora::cache_euler_deg(world, entity, "transform", Vec3::from_array(v));
                        if let Some(mut t) = world.get_mut::<Transform>(entity) {
                            t.rotation = q;
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
                        .map(|l| FieldValue::Bool(l.shadow_maps_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<DirectionalLight>(entity) {
                            l.shadow_maps_enabled = v;
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
                        .map(|l| FieldValue::Bool(l.shadow_maps_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<PointLight>(entity) {
                            l.shadow_maps_enabled = v;
                        }
                    }
                },
            },
        ],
    }
}

fn rect_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "rect_light",
        display_name: "Rect Light",
        icon: "square",
        category: "lighting",
        has_fn: |world, entity| world.get::<RectLight>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(RectLight::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<RectLight>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<RectLight>(entity).map(|l| {
                        let c = l.color.to_srgba();
                        FieldValue::Color([c.red, c.green, c.blue])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut l) = world.get_mut::<RectLight>(entity) {
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
                        .get::<RectLight>(entity)
                        .map(|l| FieldValue::Float(l.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<RectLight>(entity) {
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
                        .get::<RectLight>(entity)
                        .map(|l| FieldValue::Float(l.range))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<RectLight>(entity) {
                            l.range = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Width",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 1000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<RectLight>(entity)
                        .map(|l| FieldValue::Float(l.width))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<RectLight>(entity) {
                            l.width = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Height",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 1000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<RectLight>(entity)
                        .map(|l| FieldValue::Float(l.height))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut l) = world.get_mut::<RectLight>(entity) {
                            l.height = v;
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
                        .map(|l| FieldValue::Bool(l.shadow_maps_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut l) = world.get_mut::<SpotLight>(entity) {
                            l.shadow_maps_enabled = v;
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

/// Bevy `TextFont` (the per-entity font of any text component). The fields are
/// empty — a native drawer (`renzora_inspector`, `type_id = "text_font"`) renders
/// the font picker (bound to the shared `FontRegistry`) + size.
fn text_font_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "text_font",
        display_name: "Text Font",
        icon: "text-aa",
        category: "rendering",
        has_fn: |world, entity| world.get::<TextFont>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(TextFont::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<TextFont>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

/// Bevy rich text — the styled `TextSpan` runs under a `Text` / `Text2d`. The
/// fields are empty; a native drawer (`renzora_inspector`, `type_id =
/// "text_rich"`) lists the spans with per-run text + color + add/remove. Shown
/// whenever the entity has a text component (not separately addable).
fn text_rich_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "text_rich",
        display_name: "Rich Text",
        icon: "text-t",
        category: "rendering",
        has_fn: |world, entity| {
            world.get::<Text>(entity).is_some() || world.get::<Text2d>(entity).is_some()
        },
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

/// Bevy `LightProbe` — a reflection probe (bounds via Transform). Carries the
/// authored cubemap **Source** + **Intensity** (`ReflectionProbeSource`, which
/// is what persists) and the influence **Falloff**. The GPU
/// `GeneratedEnvironmentMapLight` is attached by `renzora_environment_map` only
/// once a valid cube is ready, so it's not edited directly here.
fn light_probe_entry() -> InspectorEntry {
    use renzora::core::ReflectionProbeSource;
    InspectorEntry {
        type_id: "light_probe",
        display_name: "Reflection Probe",
        icon: "globe",
        category: "lighting",
        has_fn: |world, entity| world.get::<LightProbe>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert((LightProbe::default(), ReflectionProbeSource::default()));
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(LightProbe, ReflectionProbeSource, GeneratedEnvironmentMapLight)>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                // Equirect `.exr`/`.hdr`/`.png` (reprojected to a POT cube) or a
                // `.ktx2`/`.dds` cube container. Stored on `ReflectionProbeSource`
                // so it persists; the GPU cube is regenerated on load.
                name: "Source (HDR / Cube)",
                field_type: FieldType::Asset {
                    extensions: vec![
                        "exr".into(),
                        "hdr".into(),
                        "png".into(),
                        "ktx2".into(),
                        "dds".into(),
                    ],
                },
                get_fn: |world, entity| {
                    let path = world
                        .get::<ReflectionProbeSource>(entity)
                        .map(|s| s.path.clone())
                        .filter(|s| !s.is_empty());
                    Some(FieldValue::Asset(path))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Asset(path) = val {
                        let intensity = world
                            .get::<ReflectionProbeSource>(entity)
                            .map(|s| s.intensity)
                            .unwrap_or(1.0);
                        world.entity_mut(entity).insert(ReflectionProbeSource {
                            path: path.unwrap_or_default(),
                            intensity,
                        });
                    }
                },
            },
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.05, min: 0.0, max: 10000.0 },
                get_fn: |world, entity| {
                    world
                        .get::<ReflectionProbeSource>(entity)
                        .map(|s| FieldValue::Float(s.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(f) = val {
                        if let Some(mut s) = world.get_mut::<ReflectionProbeSource>(entity) {
                            s.intensity = f;
                        }
                    }
                },
            },
            FieldDef {
                name: "Falloff",
                field_type: FieldType::Vec3 { speed: 0.01 },
                get_fn: |world, entity| {
                    world
                        .get::<LightProbe>(entity)
                        .map(|p| FieldValue::Vec3(p.falloff.to_array()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        if let Some(mut p) = world.get_mut::<LightProbe>(entity) {
                            p.falloff = Vec3::from_array(v);
                        }
                    }
                },
            },
        ],
    }
}

/// Bevy `ParallaxCorrection` — how a reflection probe's cubemap is box-corrected.
/// `None` = infinite (no correction); `Auto` = the probe's Transform cube;
/// `Custom` = manual half-extents (editing "Half Extents" switches to Custom).
fn parallax_correction_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "parallax_correction",
        display_name: "Parallax Correction",
        icon: "bounding-box",
        category: "lighting",
        has_fn: |world, entity| world.get::<ParallaxCorrection>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(ParallaxCorrection::Auto);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<ParallaxCorrection>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Mode",
                field_type: FieldType::Enum {
                    options: &["None", "Auto", "Custom"],
                },
                get_fn: |world, entity| {
                    world.get::<ParallaxCorrection>(entity).map(|p| {
                        FieldValue::Enum(
                            match p {
                                ParallaxCorrection::None => "None",
                                ParallaxCorrection::Auto => "Auto",
                                ParallaxCorrection::Custom(_) => "Custom",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Enum(s) = val {
                        // Preserve existing custom extents when re-selecting Custom.
                        let existing = world
                            .get::<ParallaxCorrection>(entity)
                            .and_then(|p| match p {
                                ParallaxCorrection::Custom(v) => Some(*v),
                                _ => None,
                            })
                            .unwrap_or(Vec3::splat(0.5));
                        let next = match s.as_str() {
                            "None" => ParallaxCorrection::None,
                            "Custom" => ParallaxCorrection::Custom(existing),
                            _ => ParallaxCorrection::Auto,
                        };
                        world.entity_mut(entity).insert(next);
                    }
                },
            },
            FieldDef {
                name: "Half Extents",
                field_type: FieldType::Vec3 { speed: 0.05 },
                get_fn: |world, entity| {
                    world.get::<ParallaxCorrection>(entity).map(|p| {
                        let v = match p {
                            ParallaxCorrection::Custom(v) => *v,
                            _ => Vec3::splat(0.5),
                        };
                        FieldValue::Vec3(v.to_array())
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        // Editing the extents implies Custom correction.
                        world
                            .entity_mut(entity)
                            .insert(ParallaxCorrection::Custom(Vec3::from_array(v)));
                    }
                },
            },
        ],
    }
}

/// Bevy `GeneratedEnvironmentMapLight` — the GPU side of a reflection probe,
/// attached automatically by `renzora_environment_map` once a valid cube is
/// ready (source + intensity are edited on the probe's `ReflectionProbeSource`,
/// not here). Shown when present so the niche `affects_lightmapped` toggle is
/// reachable; not directly addable (adding it with no cube fails GPU validation).
fn generated_environment_map_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "generated_environment_map_light",
        display_name: "Environment Map",
        icon: "image",
        category: "lighting",
        has_fn: |world, entity| world.get::<GeneratedEnvironmentMapLight>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Affects Lightmaps",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<GeneratedEnvironmentMapLight>(entity)
                        .map(|g| FieldValue::Bool(g.affects_lightmapped_mesh_diffuse))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if let Some(mut g) = world.get_mut::<GeneratedEnvironmentMapLight>(entity) {
                            g.affects_lightmapped_mesh_diffuse = b;
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
            // Manual exposure — a lens attribute, alongside FOV. Reads/writes the
            // camera's `Exposure.ev100`. When Auto Exposure is enabled it overrides
            // this each frame (so this is the manual value, used when AE is off).
            FieldDef {
                name: "EV100",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: -10.0,
                    max: 20.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<bevy::camera::Exposure>(entity)
                        .map(|e| FieldValue::Float(e.ev100))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut e) = world.get_mut::<bevy::camera::Exposure>(entity) {
                            e.ev100 = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Resolution",
                field_type: FieldType::Enum {
                    options: &["Full", "Half", "Quarter"],
                },
                get_fn: |world, entity| {
                    let r = world
                        .get::<renzora::core::CameraRenderResolution>(entity)
                        .map(|c| c.0)
                        .unwrap_or_default();
                    Some(FieldValue::Enum(r.label().to_string()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Enum(label) = val {
                        let r = renzora::core::viewport_types::RenderResolution::from_label(&label);
                        if let Some(mut c) =
                            world.get_mut::<renzora::core::CameraRenderResolution>(entity)
                        {
                            c.0 = r;
                        } else {
                            world
                                .entity_mut(entity)
                                .insert(renzora::core::CameraRenderResolution(r));
                        }
                    }
                },
            },
            FieldDef {
                name: "Snap to Viewport",
                field_type: FieldType::Button {
                    icon: "frame-corners",
                },
                get_fn: |_, _| None,
                set_fn: |world, entity, _| {
                    crate::camera::snap_entity_to_editor_camera(world, entity);
                },
            },
        ],
    }
}

/// Camera angle presets. The body is a native (bevy_ui) drawer registered by
/// `renzora_inspector` (`register_native_inspector_ui("camera_presets", …)`),
/// so the fields here stay empty. Shown on any 3D camera; the `CameraPresets`
/// component is inserted lazily when the first preset is captured.
fn camera_presets_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "camera_presets",
        display_name: "Camera Presets",
        icon: "video-camera",
        category: "camera",
        has_fn: |world, entity| world.get::<Camera3d>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
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
                    let l = world.get::<EnvironmentMapLight>(entity)?;
                    // Same typed-Euler cache as Transform, but under its own key so
                    // the two rotation fields on one entity don't collide.
                    let cache = world.get::<renzora::EditorEulerCache>(entity);
                    let deg =
                        renzora::rotation_euler_deg(l.rotation, cache, "environment_map_light");
                    Some(FieldValue::Vec3(deg.to_array()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3(v) = val {
                        let q = renzora::cache_euler_deg(
                            world,
                            entity,
                            "environment_map_light",
                            Vec3::from_array(v),
                        );
                        if let Some(mut l) = world.get_mut::<EnvironmentMapLight>(entity) {
                            l.rotation = q;
                        }
                    }
                },
            },
        ],
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
