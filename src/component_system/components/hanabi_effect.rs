//! Visual effects component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;
use std::path::PathBuf;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::core::{AssetBrowserState, SceneManagerState, OpenParticleFX, TabKind};
use crate::project::CurrentProject;
use crate::ui::load_effect_from_file;

use egui_phosphor::regular::SPARKLE;

// Re-export particle types for convenience
pub use crate::particles::{
    HanabiEffectData, HanabiEffectDefinition, EffectSource,
    HanabiEmitShape, ShapeDimension, SpawnMode, VelocityMode,
    BlendMode, BillboardMode, SimulationSpace, SimulationCondition,
    GradientStop,
    ParticleAlphaMode, ParticleOrientMode, ParticleColorBlendMode,
    MotionIntegrationMode, KillZone, ConformToSphere, FlipbookSettings,
};

// ============================================================================
// Hanabi GPU Particle Effect - Custom Serialize/Deserialize
// ============================================================================

fn serialize_hanabi_effect(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<HanabiEffectData>(entity)?;

    // Serialize the source
    let source_value = match &data.source {
        EffectSource::Asset { path } => {
            json!({
                "type": "asset",
                "path": path,
            })
        }
        EffectSource::Inline { definition } => {
            // Serialize the full definition
            let shape_value = match &definition.emit_shape {
                HanabiEmitShape::Point => json!({ "type": "point" }),
                HanabiEmitShape::Circle { radius, dimension } => json!({
                    "type": "circle",
                    "radius": radius,
                    "dimension": match dimension {
                        ShapeDimension::Volume => "volume",
                        ShapeDimension::Surface => "surface",
                    }
                }),
                HanabiEmitShape::Sphere { radius, dimension } => json!({
                    "type": "sphere",
                    "radius": radius,
                    "dimension": match dimension {
                        ShapeDimension::Volume => "volume",
                        ShapeDimension::Surface => "surface",
                    }
                }),
                HanabiEmitShape::Cone { base_radius, top_radius, height, dimension } => json!({
                    "type": "cone",
                    "base_radius": base_radius,
                    "top_radius": top_radius,
                    "height": height,
                    "dimension": match dimension {
                        ShapeDimension::Volume => "volume",
                        ShapeDimension::Surface => "surface",
                    }
                }),
                HanabiEmitShape::Rect { half_extents, dimension } => json!({
                    "type": "rect",
                    "half_extents": half_extents,
                    "dimension": match dimension {
                        ShapeDimension::Volume => "volume",
                        ShapeDimension::Surface => "surface",
                    }
                }),
                HanabiEmitShape::Box { half_extents } => json!({
                    "type": "box",
                    "half_extents": half_extents,
                }),
            };

            // Serialize kill zones
            let kill_zones_value: Vec<serde_json::Value> = definition.kill_zones.iter().map(|zone| {
                match zone {
                    KillZone::Sphere { center, radius, kill_inside } => json!({
                        "type": "sphere",
                        "center": center,
                        "radius": radius,
                        "kill_inside": kill_inside,
                    }),
                    KillZone::Aabb { center, half_size, kill_inside } => json!({
                        "type": "aabb",
                        "center": center,
                        "half_size": half_size,
                        "kill_inside": kill_inside,
                    }),
                }
            }).collect();

            // Serialize conform-to-sphere
            let conform_value = definition.conform_to_sphere.as_ref().map(|c| json!({
                "origin": c.origin,
                "radius": c.radius,
                "influence_dist": c.influence_dist,
                "attraction_accel": c.attraction_accel,
                "max_attraction_speed": c.max_attraction_speed,
                "shell_half_thickness": c.shell_half_thickness,
                "sticky_factor": c.sticky_factor,
            }));

            // Serialize flipbook
            let flipbook_value = definition.flipbook.as_ref().map(|fb| json!({
                "grid_columns": fb.grid_columns,
                "grid_rows": fb.grid_rows,
            }));

            // Build definition map in pieces to avoid json! recursion limit
            let mut def_map = serde_json::Map::new();
            def_map.insert("name".into(), json!(definition.name));
            def_map.insert("capacity".into(), json!(definition.capacity));
            def_map.insert("spawn_mode".into(), json!(match definition.spawn_mode {
                SpawnMode::Rate => "rate",
                SpawnMode::Burst => "burst",
                SpawnMode::BurstRate => "burst_rate",
            }));
            def_map.insert("spawn_rate".into(), json!(definition.spawn_rate));
            def_map.insert("spawn_count".into(), json!(definition.spawn_count));
            def_map.insert("spawn_duration".into(), json!(definition.spawn_duration));
            def_map.insert("spawn_cycle_count".into(), json!(definition.spawn_cycle_count));
            def_map.insert("spawn_starts_active".into(), json!(definition.spawn_starts_active));
            def_map.insert("lifetime_min".into(), json!(definition.lifetime_min));
            def_map.insert("lifetime_max".into(), json!(definition.lifetime_max));
            def_map.insert("emit_shape".into(), shape_value);
            def_map.insert("velocity_mode".into(), json!(match definition.velocity_mode {
                VelocityMode::Directional => "directional",
                VelocityMode::Radial => "radial",
                VelocityMode::Tangent => "tangent",
                VelocityMode::Random => "random",
            }));
            def_map.insert("velocity_magnitude".into(), json!(definition.velocity_magnitude));
            def_map.insert("velocity_spread".into(), json!(definition.velocity_spread));
            def_map.insert("velocity_direction".into(), json!(definition.velocity_direction));
            def_map.insert("velocity_speed_min".into(), json!(definition.velocity_speed_min));
            def_map.insert("velocity_speed_max".into(), json!(definition.velocity_speed_max));
            def_map.insert("velocity_axis".into(), json!(definition.velocity_axis));
            def_map.insert("acceleration".into(), json!(definition.acceleration));
            def_map.insert("linear_drag".into(), json!(definition.linear_drag));
            def_map.insert("radial_acceleration".into(), json!(definition.radial_acceleration));
            def_map.insert("tangent_acceleration".into(), json!(definition.tangent_acceleration));
            def_map.insert("tangent_accel_axis".into(), json!(definition.tangent_accel_axis));
            def_map.insert("conform_to_sphere".into(), json!(conform_value));
            def_map.insert("size_start".into(), json!(definition.size_start));
            def_map.insert("size_end".into(), json!(definition.size_end));
            def_map.insert("size_start_min".into(), json!(definition.size_start_min));
            def_map.insert("size_start_max".into(), json!(definition.size_start_max));
            def_map.insert("size_non_uniform".into(), json!(definition.size_non_uniform));
            def_map.insert("size_start_x".into(), json!(definition.size_start_x));
            def_map.insert("size_start_y".into(), json!(definition.size_start_y));
            def_map.insert("size_end_x".into(), json!(definition.size_end_x));
            def_map.insert("size_end_y".into(), json!(definition.size_end_y));
            def_map.insert("screen_space_size".into(), json!(definition.screen_space_size));
            def_map.insert("roundness".into(), json!(definition.roundness));
            let gradient_value: Vec<serde_json::Value> = definition.color_gradient.iter().map(|stop| {
                json!({ "position": stop.position, "color": stop.color })
            }).collect();
            def_map.insert("color_gradient".into(), json!(gradient_value));
            def_map.insert("use_flat_color".into(), json!(definition.use_flat_color));
            def_map.insert("flat_color".into(), json!(definition.flat_color));
            def_map.insert("use_hdr_color".into(), json!(definition.use_hdr_color));
            def_map.insert("hdr_intensity".into(), json!(definition.hdr_intensity));
            def_map.insert("color_blend_mode".into(), json!(match definition.color_blend_mode {
                ParticleColorBlendMode::Modulate => "modulate",
                ParticleColorBlendMode::Overwrite => "overwrite",
                ParticleColorBlendMode::Add => "add",
            }));
            def_map.insert("blend_mode".into(), json!(match definition.blend_mode {
                BlendMode::Blend => "blend",
                BlendMode::Additive => "additive",
                BlendMode::Multiply => "multiply",
            }));
            def_map.insert("texture_path".into(), json!(definition.texture_path));
            def_map.insert("billboard_mode".into(), json!(match definition.billboard_mode {
                BillboardMode::FaceCamera => "face_camera",
                BillboardMode::FaceCameraY => "face_camera_y",
                BillboardMode::Velocity => "velocity",
                BillboardMode::Fixed => "fixed",
            }));
            def_map.insert("render_layer".into(), json!(definition.render_layer));
            def_map.insert("alpha_mode".into(), json!(match definition.alpha_mode {
                ParticleAlphaMode::Blend => "blend",
                ParticleAlphaMode::Premultiply => "premultiply",
                ParticleAlphaMode::Add => "add",
                ParticleAlphaMode::Multiply => "multiply",
                ParticleAlphaMode::Mask => "mask",
                ParticleAlphaMode::Opaque => "opaque",
            }));
            def_map.insert("alpha_mask_threshold".into(), json!(definition.alpha_mask_threshold));
            def_map.insert("orient_mode".into(), json!(match definition.orient_mode {
                ParticleOrientMode::ParallelCameraDepthPlane => "parallel_camera",
                ParticleOrientMode::FaceCameraPosition => "face_camera",
                ParticleOrientMode::AlongVelocity => "along_velocity",
            }));
            def_map.insert("rotation_speed".into(), json!(definition.rotation_speed));
            def_map.insert("flipbook".into(), json!(flipbook_value));
            def_map.insert("simulation_space".into(), json!(match definition.simulation_space {
                SimulationSpace::Local => "local",
                SimulationSpace::World => "world",
            }));
            def_map.insert("simulation_condition".into(), json!(match definition.simulation_condition {
                SimulationCondition::Always => "always",
                SimulationCondition::WhenVisible => "when_visible",
            }));
            def_map.insert("motion_integration".into(), json!(match definition.motion_integration {
                MotionIntegrationMode::PostUpdate => "post_update",
                MotionIntegrationMode::PreUpdate => "pre_update",
                MotionIntegrationMode::None => "none",
            }));
            def_map.insert("kill_zones".into(), json!(kill_zones_value));

            json!({
                "type": "inline",
                "definition": serde_json::Value::Object(def_map),
            })
        }
    };

    Some(json!({
        "source": source_value,
        "playing": data.playing,
        "rate_multiplier": data.rate_multiplier,
        "scale_multiplier": data.scale_multiplier,
        "color_tint": data.color_tint,
        "time_scale": data.time_scale,
    }))
}

fn deserialize_hanabi_effect(entity_commands: &mut EntityCommands, data: &serde_json::Value, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) {
    let source = if let Some(source_data) = data.get("source") {
        let source_type = source_data.get("type").and_then(|v| v.as_str()).unwrap_or("inline");
        match source_type {
            "asset" => {
                let path = source_data.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                EffectSource::Asset { path }
            }
            "inline" | _ => {
                if let Some(def_data) = source_data.get("definition") {
                    let emit_shape = if let Some(shape_data) = def_data.get("emit_shape") {
                        let shape_type = shape_data.get("type").and_then(|v| v.as_str()).unwrap_or("point");
                        match shape_type {
                            "circle" => HanabiEmitShape::Circle {
                                radius: shape_data.get("radius").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                                dimension: match shape_data.get("dimension").and_then(|v| v.as_str()).unwrap_or("volume") {
                                    "surface" => ShapeDimension::Surface,
                                    _ => ShapeDimension::Volume,
                                },
                            },
                            "sphere" => HanabiEmitShape::Sphere {
                                radius: shape_data.get("radius").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                                dimension: match shape_data.get("dimension").and_then(|v| v.as_str()).unwrap_or("volume") {
                                    "surface" => ShapeDimension::Surface,
                                    _ => ShapeDimension::Volume,
                                },
                            },
                            "cone" => HanabiEmitShape::Cone {
                                base_radius: shape_data.get("base_radius").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
                                top_radius: shape_data.get("top_radius").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                                height: shape_data.get("height").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                                dimension: match shape_data.get("dimension").and_then(|v| v.as_str()).unwrap_or("volume") {
                                    "surface" => ShapeDimension::Surface,
                                    _ => ShapeDimension::Volume,
                                },
                            },
                            "rect" => HanabiEmitShape::Rect {
                                half_extents: shape_data.get("half_extents").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0]),
                                dimension: match shape_data.get("dimension").and_then(|v| v.as_str()).unwrap_or("volume") {
                                    "surface" => ShapeDimension::Surface,
                                    _ => ShapeDimension::Volume,
                                },
                            },
                            "box" => HanabiEmitShape::Box {
                                half_extents: shape_data.get("half_extents").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0, 1.0]),
                            },
                            _ => HanabiEmitShape::Point,
                        }
                    } else {
                        HanabiEmitShape::Point
                    };

                    let color_gradient = if let Some(gradient_data) = def_data.get("color_gradient") {
                        gradient_data.as_array().map(|arr| {
                            arr.iter().filter_map(|stop| {
                                Some(GradientStop {
                                    position: stop.get("position").and_then(|v| v.as_f64())? as f32,
                                    color: stop.get("color").and_then(|v| serde_json::from_value(v.clone()).ok())?,
                                })
                            }).collect()
                        }).unwrap_or_else(|| vec![
                            GradientStop { position: 0.0, color: [1.0, 1.0, 1.0, 1.0] },
                            GradientStop { position: 1.0, color: [1.0, 1.0, 1.0, 0.0] },
                        ])
                    } else {
                        vec![
                            GradientStop { position: 0.0, color: [1.0, 1.0, 1.0, 1.0] },
                            GradientStop { position: 1.0, color: [1.0, 1.0, 1.0, 0.0] },
                        ]
                    };

                    // Deserialize kill zones
                    let kill_zones = if let Some(zones_data) = def_data.get("kill_zones") {
                        zones_data.as_array().map(|arr| {
                            arr.iter().filter_map(|zone| {
                                let zone_type = zone.get("type").and_then(|v| v.as_str())?;
                                match zone_type {
                                    "sphere" => Some(KillZone::Sphere {
                                        center: zone.get("center").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 0.0, 0.0]),
                                        radius: zone.get("radius").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
                                        kill_inside: zone.get("kill_inside").and_then(|v| v.as_bool()).unwrap_or(false),
                                    }),
                                    "aabb" => Some(KillZone::Aabb {
                                        center: zone.get("center").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 0.0, 0.0]),
                                        half_size: zone.get("half_size").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([5.0, 5.0, 5.0]),
                                        kill_inside: zone.get("kill_inside").and_then(|v| v.as_bool()).unwrap_or(false),
                                    }),
                                    _ => None,
                                }
                            }).collect()
                        }).unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    // Deserialize conform-to-sphere
                    let conform_to_sphere = def_data.get("conform_to_sphere").and_then(|v| {
                        if v.is_null() { return None; }
                        Some(ConformToSphere {
                            origin: v.get("origin").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 0.0, 0.0]),
                            radius: v.get("radius").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                            influence_dist: v.get("influence_dist").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32,
                            attraction_accel: v.get("attraction_accel").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32,
                            max_attraction_speed: v.get("max_attraction_speed").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
                            shell_half_thickness: v.get("shell_half_thickness").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
                            sticky_factor: v.get("sticky_factor").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
                        })
                    });

                    // Deserialize flipbook
                    let flipbook = def_data.get("flipbook").and_then(|v| {
                        if v.is_null() { return None; }
                        Some(FlipbookSettings {
                            grid_columns: v.get("grid_columns").and_then(|v| v.as_u64()).unwrap_or(4) as u32,
                            grid_rows: v.get("grid_rows").and_then(|v| v.as_u64()).unwrap_or(4) as u32,
                        })
                    });

                    let definition = HanabiEffectDefinition {
                        name: def_data.get("name").and_then(|v| v.as_str()).unwrap_or("Effect").to_string(),
                        capacity: def_data.get("capacity").and_then(|v| v.as_u64()).unwrap_or(1000) as u32,
                        spawn_mode: match def_data.get("spawn_mode").and_then(|v| v.as_str()).unwrap_or("rate") {
                            "burst" => SpawnMode::Burst,
                            "burst_rate" => SpawnMode::BurstRate,
                            _ => SpawnMode::Rate,
                        },
                        spawn_rate: def_data.get("spawn_rate").and_then(|v| v.as_f64()).unwrap_or(50.0) as f32,
                        spawn_count: def_data.get("spawn_count").and_then(|v| v.as_u64()).unwrap_or(10) as u32,
                        spawn_duration: def_data.get("spawn_duration").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        spawn_cycle_count: def_data.get("spawn_cycle_count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                        spawn_starts_active: def_data.get("spawn_starts_active").and_then(|v| v.as_bool()).unwrap_or(true),
                        lifetime_min: def_data.get("lifetime_min").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        lifetime_max: def_data.get("lifetime_max").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
                        emit_shape,
                        velocity_mode: match def_data.get("velocity_mode").and_then(|v| v.as_str()).unwrap_or("directional") {
                            "radial" => VelocityMode::Radial,
                            "tangent" => VelocityMode::Tangent,
                            "random" => VelocityMode::Random,
                            _ => VelocityMode::Directional,
                        },
                        velocity_magnitude: def_data.get("velocity_magnitude").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
                        velocity_spread: def_data.get("velocity_spread").and_then(|v| v.as_f64()).unwrap_or(0.3) as f32,
                        velocity_direction: def_data.get("velocity_direction").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 1.0, 0.0]),
                        velocity_speed_min: def_data.get("velocity_speed_min").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        velocity_speed_max: def_data.get("velocity_speed_max").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        velocity_axis: def_data.get("velocity_axis").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 1.0, 0.0]),
                        acceleration: def_data.get("acceleration").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, -2.0, 0.0]),
                        linear_drag: def_data.get("linear_drag").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        radial_acceleration: def_data.get("radial_acceleration").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        tangent_acceleration: def_data.get("tangent_acceleration").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        tangent_accel_axis: def_data.get("tangent_accel_axis").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, 1.0, 0.0]),
                        conform_to_sphere,
                        size_start: def_data.get("size_start").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
                        size_end: def_data.get("size_end").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        size_curve: Vec::new(),
                        size_start_min: def_data.get("size_start_min").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        size_start_max: def_data.get("size_start_max").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        size_non_uniform: def_data.get("size_non_uniform").and_then(|v| v.as_bool()).unwrap_or(false),
                        size_start_x: def_data.get("size_start_x").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
                        size_start_y: def_data.get("size_start_y").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
                        size_end_x: def_data.get("size_end_x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        size_end_y: def_data.get("size_end_y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        screen_space_size: def_data.get("screen_space_size").and_then(|v| v.as_bool()).unwrap_or(false),
                        roundness: def_data.get("roundness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        color_gradient,
                        use_flat_color: def_data.get("use_flat_color").and_then(|v| v.as_bool()).unwrap_or(false),
                        flat_color: def_data.get("flat_color").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0, 1.0, 1.0]),
                        use_hdr_color: def_data.get("use_hdr_color").and_then(|v| v.as_bool()).unwrap_or(false),
                        hdr_intensity: def_data.get("hdr_intensity").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color_blend_mode: match def_data.get("color_blend_mode").and_then(|v| v.as_str()).unwrap_or("modulate") {
                            "overwrite" => ParticleColorBlendMode::Overwrite,
                            "add" => ParticleColorBlendMode::Add,
                            _ => ParticleColorBlendMode::Modulate,
                        },
                        blend_mode: match def_data.get("blend_mode").and_then(|v| v.as_str()).unwrap_or("blend") {
                            "additive" => BlendMode::Additive,
                            "multiply" => BlendMode::Multiply,
                            _ => BlendMode::Blend,
                        },
                        texture_path: def_data.get("texture_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        billboard_mode: match def_data.get("billboard_mode").and_then(|v| v.as_str()).unwrap_or("face_camera") {
                            "face_camera_y" => BillboardMode::FaceCameraY,
                            "velocity" => BillboardMode::Velocity,
                            "fixed" => BillboardMode::Fixed,
                            _ => BillboardMode::FaceCamera,
                        },
                        render_layer: def_data.get("render_layer").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                        alpha_mode: match def_data.get("alpha_mode").and_then(|v| v.as_str()).unwrap_or("blend") {
                            "premultiply" => ParticleAlphaMode::Premultiply,
                            "add" => ParticleAlphaMode::Add,
                            "multiply" => ParticleAlphaMode::Multiply,
                            "mask" => ParticleAlphaMode::Mask,
                            "opaque" => ParticleAlphaMode::Opaque,
                            _ => ParticleAlphaMode::Blend,
                        },
                        alpha_mask_threshold: def_data.get("alpha_mask_threshold").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
                        orient_mode: match def_data.get("orient_mode").and_then(|v| v.as_str()).unwrap_or("parallel_camera") {
                            "face_camera" => ParticleOrientMode::FaceCameraPosition,
                            "along_velocity" => ParticleOrientMode::AlongVelocity,
                            _ => ParticleOrientMode::ParallelCameraDepthPlane,
                        },
                        rotation_speed: def_data.get("rotation_speed").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        flipbook,
                        simulation_space: match def_data.get("simulation_space").and_then(|v| v.as_str()).unwrap_or("local") {
                            "world" => SimulationSpace::World,
                            _ => SimulationSpace::Local,
                        },
                        simulation_condition: match def_data.get("simulation_condition").and_then(|v| v.as_str()).unwrap_or("always") {
                            "when_visible" => SimulationCondition::WhenVisible,
                            _ => SimulationCondition::Always,
                        },
                        motion_integration: match def_data.get("motion_integration").and_then(|v| v.as_str()).unwrap_or("post_update") {
                            "pre_update" => MotionIntegrationMode::PreUpdate,
                            "none" => MotionIntegrationMode::None,
                            _ => MotionIntegrationMode::PostUpdate,
                        },
                        kill_zones,
                        variables: std::collections::HashMap::new(),
                    };
                    EffectSource::Inline { definition }
                } else {
                    EffectSource::default()
                }
            }
        }
    } else {
        EffectSource::default()
    };

    let effect_data = HanabiEffectData {
        source,
        playing: data.get("playing").and_then(|v| v.as_bool()).unwrap_or(true),
        rate_multiplier: data.get("rate_multiplier").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        scale_multiplier: data.get("scale_multiplier").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        color_tint: data.get("color_tint").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([1.0, 1.0, 1.0, 1.0]),
        time_scale: data.get("time_scale").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        variable_overrides: std::collections::HashMap::new(),
    };
    entity_commands.insert(effect_data);
}

// ============================================================================
// Hanabi GPU Particle Effect - Custom Inspector
// ============================================================================

fn inspect_hanabi_effect(ui: &mut egui::Ui, world: &mut World, entity: Entity, _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>) -> bool {
    let mut changed = false;

    // Check for drag-and-drop of .effect files
    let dragging_effect_path = {
        if let Some(assets) = world.get_resource::<AssetBrowserState>() {
            assets.dragging_asset.as_ref()
                .filter(|p| p.to_string_lossy().to_lowercase().ends_with(".effect"))
                .cloned()
        } else {
            None
        }
    };

    // Get project path for resolving asset paths
    let project_assets_path = world.get_resource::<CurrentProject>()
        .map(|p| p.path.join("assets"));

    let Some(mut data) = world.get_mut::<HanabiEffectData>(entity) else {
        return false;
    };

    // Runtime controls section
    ui.collapsing("Runtime Controls", |ui| {
        if ui.checkbox(&mut data.playing, "Playing").changed() {
            changed = true;
        }

        ui.horizontal(|ui| {
            ui.label("Rate Multiplier:");
            if ui.add(egui::DragValue::new(&mut data.rate_multiplier).speed(0.05).range(0.0..=10.0)).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Scale Multiplier:");
            if ui.add(egui::DragValue::new(&mut data.scale_multiplier).speed(0.05).range(0.0..=10.0)).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Time Scale:");
            if ui.add(egui::DragValue::new(&mut data.time_scale).speed(0.05).range(0.0..=5.0)).changed() {
                changed = true;
            }
        });

        let mut tint = egui::Rgba::from_rgba_premultiplied(
            data.color_tint[0], data.color_tint[1], data.color_tint[2], data.color_tint[3]
        );
        ui.horizontal(|ui| {
            ui.label("Color Tint:");
            if egui::color_picker::color_edit_button_rgba(ui, &mut tint, egui::color_picker::Alpha::OnlyBlend).changed() {
                data.color_tint = [tint.r(), tint.g(), tint.b(), tint.a()];
                changed = true;
            }
        });
    });

    // Source section
    ui.separator();
    ui.collapsing("Effect Source", |ui| {
        let is_asset = matches!(&data.source, EffectSource::Asset { .. });

        ui.horizontal(|ui| {
            ui.label("Source Type:");
            if ui.radio(is_asset, "Asset").clicked() && !is_asset {
                data.source = EffectSource::Asset { path: String::new() };
                changed = true;
            }
            if ui.radio(!is_asset, "Inline").clicked() && is_asset {
                data.source = EffectSource::Inline { definition: HanabiEffectDefinition::default() };
                changed = true;
            }
        });

        match &mut data.source {
            EffectSource::Asset { path } => {
                ui.horizontal(|ui| {
                    ui.label("Asset Path:");
                    if ui.text_edit_singleline(path).changed() {
                        changed = true;
                    }
                });

                // Drop zone for .effect files
                let drop_rect = ui.allocate_space(egui::vec2(ui.available_width(), 28.0)).1;
                let is_hovering = dragging_effect_path.is_some() && ui.input(|i| {
                    i.pointer.interact_pos().is_some_and(|pos| drop_rect.contains(pos))
                });

                let drop_color = if is_hovering {
                    egui::Color32::from_rgb(80, 140, 200)
                } else if dragging_effect_path.is_some() {
                    egui::Color32::from_rgb(60, 100, 140)
                } else {
                    egui::Color32::from_rgb(50, 52, 58)
                };

                ui.painter().rect_stroke(drop_rect, 4.0, egui::Stroke::new(1.0, drop_color), egui::StrokeKind::Inside);
                ui.painter().text(
                    drop_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Drop .effect file here",
                    egui::FontId::proportional(11.0),
                    drop_color,
                );

                // Handle drop
                if is_hovering && ui.input(|i| i.pointer.any_released()) {
                    if let Some(ref dragged_path) = dragging_effect_path {
                        // Compute relative path from project assets folder
                        let relative = if let Some(ref assets_base) = project_assets_path {
                            dragged_path.strip_prefix(assets_base)
                                .map(|r| r.to_string_lossy().to_string())
                                .unwrap_or_else(|_| dragged_path.to_string_lossy().to_string())
                        } else {
                            dragged_path.to_string_lossy().to_string()
                        };
                        *path = relative;
                        changed = true;
                    }
                }
            }
            EffectSource::Inline { definition } => {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    if ui.text_edit_singleline(&mut definition.name).changed() {
                        changed = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Capacity:");
                    if ui.add(egui::DragValue::new(&mut definition.capacity).speed(10.0).range(1..=100000)).changed() {
                        changed = true;
                    }
                });

                ui.small("Use the Particle Editor panel for full editing");
            }
        }
    });

    // For asset sources, show read-only settings and "Open in Particle Editor" button
    let asset_path_for_display = match &data.source {
        EffectSource::Asset { path } if !path.is_empty() => Some(path.clone()),
        _ => None,
    };

    // Need to drop the mutable borrow on data before accessing world resources
    drop(data);

    if let Some(asset_path) = asset_path_for_display {
        // Resolve the full path and load the definition for display
        let full_path = if let Some(ref assets_base) = project_assets_path {
            assets_base.join(&asset_path)
        } else {
            PathBuf::from(&asset_path)
        };

        // "Open in Particle Editor" button
        if ui.button("Open in Particle Editor").clicked() {
            if let Some(mut scene_state) = world.get_resource_mut::<SceneManagerState>() {
                // Check if already open
                let mut found = false;
                for (idx, particle) in scene_state.open_particles.iter().enumerate() {
                    if particle.path == full_path {
                        scene_state.set_active_document(TabKind::ParticleFX(idx));
                        found = true;
                        break;
                    }
                }
                if !found {
                    let name = full_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let new_idx = scene_state.open_particles.len();
                    scene_state.open_particles.push(OpenParticleFX {
                        path: full_path.clone(),
                        name,
                        is_modified: false,
                    });
                    scene_state.tab_order.push(TabKind::ParticleFX(new_idx));
                    scene_state.set_active_document(TabKind::ParticleFX(new_idx));
                }
            }
        }

        // Load and display read-only settings
        if let Some(def) = load_effect_from_file(&full_path) {
            ui.separator();
            ui.collapsing("Effect Settings (read-only)", |ui| {
                ui.label(format!("Name: {}", def.name));
                ui.label(format!("Capacity: {}", def.capacity));

                ui.collapsing("Spawning", |ui| {
                    ui.label(format!("Mode: {:?}", def.spawn_mode));
                    ui.label(format!("Rate: {:.1}/s", def.spawn_rate));
                    ui.label(format!("Count: {}", def.spawn_count));
                    ui.label(format!("Duration: {:.1}s", def.spawn_duration));
                    ui.label(format!("Lifetime: {:.2} - {:.2}s", def.lifetime_min, def.lifetime_max));
                });

                ui.collapsing("Velocity", |ui| {
                    ui.label(format!("Mode: {:?}", def.velocity_mode));
                    ui.label(format!("Magnitude: {:.2}", def.velocity_magnitude));
                    ui.label(format!("Spread: {:.2}", def.velocity_spread));
                    ui.label(format!("Direction: [{:.2}, {:.2}, {:.2}]",
                        def.velocity_direction[0], def.velocity_direction[1], def.velocity_direction[2]));
                });

                ui.collapsing("Forces", |ui| {
                    ui.label(format!("Acceleration: [{:.2}, {:.2}, {:.2}]",
                        def.acceleration[0], def.acceleration[1], def.acceleration[2]));
                    ui.label(format!("Linear Drag: {:.2}", def.linear_drag));
                    ui.label(format!("Radial Accel: {:.2}", def.radial_acceleration));
                    ui.label(format!("Tangent Accel: {:.2}", def.tangent_acceleration));
                });

                ui.collapsing("Size", |ui| {
                    ui.label(format!("Start: {:.3}", def.size_start));
                    ui.label(format!("End: {:.3}", def.size_end));
                    if def.size_non_uniform {
                        ui.label(format!("Non-uniform: X={:.3}/{:.3}, Y={:.3}/{:.3}",
                            def.size_start_x, def.size_end_x, def.size_start_y, def.size_end_y));
                    }
                });

                ui.collapsing("Color", |ui| {
                    if def.use_flat_color {
                        ui.label(format!("Flat: [{:.2}, {:.2}, {:.2}, {:.2}]",
                            def.flat_color[0], def.flat_color[1], def.flat_color[2], def.flat_color[3]));
                    } else {
                        ui.label(format!("Gradient stops: {}", def.color_gradient.len()));
                    }
                    ui.label(format!("Blend mode: {:?}", def.color_blend_mode));
                });

                ui.collapsing("Rendering", |ui| {
                    ui.label(format!("Alpha mode: {:?}", def.alpha_mode));
                    ui.label(format!("Orient mode: {:?}", def.orient_mode));
                    ui.label(format!("Render layer: {}", def.render_layer));
                    if let Some(ref tex) = def.texture_path {
                        ui.label(format!("Texture: {}", tex));
                    }
                });

                ui.collapsing("Simulation", |ui| {
                    ui.label(format!("Space: {:?}", def.simulation_space));
                    ui.label(format!("Condition: {:?}", def.simulation_condition));
                    ui.label(format!("Integration: {:?}", def.motion_integration));
                    ui.label(format!("Emit shape: {:?}", def.emit_shape));
                });
            });
        }
    }

    // Clear dragging_asset if we accepted a drop
    if changed {
        if let Some(ref _dragged) = dragging_effect_path {
            if let Some(mut assets) = world.get_resource_mut::<AssetBrowserState>() {
                assets.dragging_asset = None;
            }
        }
    }

    changed
}

// ============================================================================
// Registration
// ============================================================================

/// Register all effects components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(HanabiEffectData {
        type_id: "hanabi_effect",
        display_name: "Hanabi Effect",
        category: ComponentCategory::Effects,
        icon: SPARKLE,
        priority: 6,
        custom_inspector: inspect_hanabi_effect,
        custom_serialize: serialize_hanabi_effect,
        custom_deserialize: deserialize_hanabi_effect,
    }));
}
