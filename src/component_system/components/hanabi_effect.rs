//! Visual effects component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;

use egui_phosphor::regular::SPARKLE;

// Re-export particle types for convenience
pub use crate::particles::{
    HanabiEffectData, HanabiEffectDefinition, EffectSource,
    HanabiEmitShape, ShapeDimension, SpawnMode, VelocityMode,
    BlendMode, BillboardMode, SimulationSpace, SimulationCondition,
    GradientStop, CurvePoint, EffectVariable,
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

            json!({
                "type": "inline",
                "definition": {
                    "name": definition.name,
                    "capacity": definition.capacity,
                    "spawn_mode": match definition.spawn_mode {
                        SpawnMode::Rate => "rate",
                        SpawnMode::Burst => "burst",
                        SpawnMode::BurstRate => "burst_rate",
                    },
                    "spawn_rate": definition.spawn_rate,
                    "spawn_count": definition.spawn_count,
                    "lifetime_min": definition.lifetime_min,
                    "lifetime_max": definition.lifetime_max,
                    "emit_shape": shape_value,
                    "velocity_mode": match definition.velocity_mode {
                        VelocityMode::Directional => "directional",
                        VelocityMode::Radial => "radial",
                        VelocityMode::Tangent => "tangent",
                        VelocityMode::Random => "random",
                    },
                    "velocity_magnitude": definition.velocity_magnitude,
                    "velocity_spread": definition.velocity_spread,
                    "velocity_direction": definition.velocity_direction,
                    "acceleration": definition.acceleration,
                    "linear_drag": definition.linear_drag,
                    "radial_acceleration": definition.radial_acceleration,
                    "tangent_acceleration": definition.tangent_acceleration,
                    "size_start": definition.size_start,
                    "size_end": definition.size_end,
                    "color_gradient": definition.color_gradient.iter().map(|stop| {
                        json!({
                            "position": stop.position,
                            "color": stop.color,
                        })
                    }).collect::<Vec<_>>(),
                    "blend_mode": match definition.blend_mode {
                        BlendMode::Blend => "blend",
                        BlendMode::Additive => "additive",
                        BlendMode::Multiply => "multiply",
                    },
                    "texture_path": definition.texture_path,
                    "billboard_mode": match definition.billboard_mode {
                        BillboardMode::FaceCamera => "face_camera",
                        BillboardMode::FaceCameraY => "face_camera_y",
                        BillboardMode::Velocity => "velocity",
                        BillboardMode::Fixed => "fixed",
                    },
                    "render_layer": definition.render_layer,
                    "simulation_space": match definition.simulation_space {
                        SimulationSpace::Local => "local",
                        SimulationSpace::World => "world",
                    },
                    "simulation_condition": match definition.simulation_condition {
                        SimulationCondition::Always => "always",
                        SimulationCondition::WhenVisible => "when_visible",
                    },
                }
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
                        acceleration: def_data.get("acceleration").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or([0.0, -2.0, 0.0]),
                        linear_drag: def_data.get("linear_drag").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        radial_acceleration: def_data.get("radial_acceleration").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        tangent_acceleration: def_data.get("tangent_acceleration").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        size_start: def_data.get("size_start").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32,
                        size_end: def_data.get("size_end").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        size_curve: Vec::new(),
                        color_gradient,
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
                        simulation_space: match def_data.get("simulation_space").and_then(|v| v.as_str()).unwrap_or("local") {
                            "world" => SimulationSpace::World,
                            _ => SimulationSpace::Local,
                        },
                        simulation_condition: match def_data.get("simulation_condition").and_then(|v| v.as_str()).unwrap_or("always") {
                            "when_visible" => SimulationCondition::WhenVisible,
                            _ => SimulationCondition::Always,
                        },
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
                ui.small("Drop an .effect file here or enter path");
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

                // Spawning section
                ui.collapsing("Spawning", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Mode:");
                        egui::ComboBox::from_id_salt("spawn_mode")
                            .selected_text(match definition.spawn_mode {
                                SpawnMode::Rate => "Rate",
                                SpawnMode::Burst => "Burst",
                                SpawnMode::BurstRate => "Burst Rate",
                            })
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(definition.spawn_mode == SpawnMode::Rate, "Rate").clicked() {
                                    definition.spawn_mode = SpawnMode::Rate;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.spawn_mode == SpawnMode::Burst, "Burst").clicked() {
                                    definition.spawn_mode = SpawnMode::Burst;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.spawn_mode == SpawnMode::BurstRate, "Burst Rate").clicked() {
                                    definition.spawn_mode = SpawnMode::BurstRate;
                                    changed = true;
                                }
                            });
                    });

                    if definition.spawn_mode == SpawnMode::Rate || definition.spawn_mode == SpawnMode::BurstRate {
                        ui.horizontal(|ui| {
                            ui.label("Rate (per sec):");
                            if ui.add(egui::DragValue::new(&mut definition.spawn_rate).speed(1.0).range(0.1..=10000.0)).changed() {
                                changed = true;
                            }
                        });
                    }

                    if definition.spawn_mode == SpawnMode::Burst || definition.spawn_mode == SpawnMode::BurstRate {
                        ui.horizontal(|ui| {
                            ui.label("Count:");
                            if ui.add(egui::DragValue::new(&mut definition.spawn_count).speed(1.0).range(1..=10000)).changed() {
                                changed = true;
                            }
                        });
                    }
                });

                // Lifetime section
                ui.collapsing("Lifetime", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Min:");
                        if ui.add(egui::DragValue::new(&mut definition.lifetime_min).speed(0.1).range(0.01..=60.0).suffix("s")).changed() {
                            changed = true;
                        }
                        ui.label("Max:");
                        if ui.add(egui::DragValue::new(&mut definition.lifetime_max).speed(0.1).range(0.01..=60.0).suffix("s")).changed() {
                            changed = true;
                        }
                    });
                });

                // Size section
                ui.collapsing("Size", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Start:");
                        if ui.add(egui::DragValue::new(&mut definition.size_start).speed(0.01).range(0.001..=10.0)).changed() {
                            changed = true;
                        }
                        ui.label("End:");
                        if ui.add(egui::DragValue::new(&mut definition.size_end).speed(0.01).range(0.0..=10.0)).changed() {
                            changed = true;
                        }
                    });
                });

                // Velocity section
                ui.collapsing("Velocity", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Mode:");
                        egui::ComboBox::from_id_salt("velocity_mode")
                            .selected_text(match definition.velocity_mode {
                                VelocityMode::Directional => "Directional",
                                VelocityMode::Radial => "Radial",
                                VelocityMode::Tangent => "Tangent",
                                VelocityMode::Random => "Random",
                            })
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(definition.velocity_mode == VelocityMode::Directional, "Directional").clicked() {
                                    definition.velocity_mode = VelocityMode::Directional;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.velocity_mode == VelocityMode::Radial, "Radial").clicked() {
                                    definition.velocity_mode = VelocityMode::Radial;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.velocity_mode == VelocityMode::Tangent, "Tangent").clicked() {
                                    definition.velocity_mode = VelocityMode::Tangent;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.velocity_mode == VelocityMode::Random, "Random").clicked() {
                                    definition.velocity_mode = VelocityMode::Random;
                                    changed = true;
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Magnitude:");
                        if ui.add(egui::DragValue::new(&mut definition.velocity_magnitude).speed(0.1).range(0.0..=100.0)).changed() {
                            changed = true;
                        }
                    });

                    if definition.velocity_mode == VelocityMode::Directional {
                        ui.horizontal(|ui| {
                            ui.label("Spread:");
                            if ui.add(egui::DragValue::new(&mut definition.velocity_spread).speed(0.05).range(0.0..=std::f32::consts::PI)).changed() {
                                changed = true;
                            }
                        });

                        ui.label("Direction:");
                        ui.horizontal(|ui| {
                            ui.label("X:");
                            if ui.add(egui::DragValue::new(&mut definition.velocity_direction[0]).speed(0.1).range(-1.0..=1.0)).changed() {
                                changed = true;
                            }
                            ui.label("Y:");
                            if ui.add(egui::DragValue::new(&mut definition.velocity_direction[1]).speed(0.1).range(-1.0..=1.0)).changed() {
                                changed = true;
                            }
                            ui.label("Z:");
                            if ui.add(egui::DragValue::new(&mut definition.velocity_direction[2]).speed(0.1).range(-1.0..=1.0)).changed() {
                                changed = true;
                            }
                        });
                    }
                });

                // Forces section
                ui.collapsing("Forces", |ui| {
                    ui.label("Acceleration:");
                    ui.horizontal(|ui| {
                        ui.label("X:");
                        if ui.add(egui::DragValue::new(&mut definition.acceleration[0]).speed(0.1)).changed() {
                            changed = true;
                        }
                        ui.label("Y:");
                        if ui.add(egui::DragValue::new(&mut definition.acceleration[1]).speed(0.1)).changed() {
                            changed = true;
                        }
                        ui.label("Z:");
                        if ui.add(egui::DragValue::new(&mut definition.acceleration[2]).speed(0.1)).changed() {
                            changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Linear Drag:");
                        if ui.add(egui::DragValue::new(&mut definition.linear_drag).speed(0.05).range(0.0..=10.0)).changed() {
                            changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Radial Accel:");
                        if ui.add(egui::DragValue::new(&mut definition.radial_acceleration).speed(0.1).range(-100.0..=100.0)).changed() {
                            changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Tangent Accel:");
                        if ui.add(egui::DragValue::new(&mut definition.tangent_acceleration).speed(0.1).range(-100.0..=100.0)).changed() {
                            changed = true;
                        }
                    });
                });

                // Rendering section
                ui.collapsing("Rendering", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Blend Mode:");
                        egui::ComboBox::from_id_salt("blend_mode")
                            .selected_text(match definition.blend_mode {
                                BlendMode::Blend => "Blend",
                                BlendMode::Additive => "Additive",
                                BlendMode::Multiply => "Multiply",
                            })
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(definition.blend_mode == BlendMode::Blend, "Blend").clicked() {
                                    definition.blend_mode = BlendMode::Blend;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.blend_mode == BlendMode::Additive, "Additive").clicked() {
                                    definition.blend_mode = BlendMode::Additive;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.blend_mode == BlendMode::Multiply, "Multiply").clicked() {
                                    definition.blend_mode = BlendMode::Multiply;
                                    changed = true;
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Billboard:");
                        egui::ComboBox::from_id_salt("billboard_mode")
                            .selected_text(match definition.billboard_mode {
                                BillboardMode::FaceCamera => "Face Camera",
                                BillboardMode::FaceCameraY => "Face Camera (Y Lock)",
                                BillboardMode::Velocity => "Velocity",
                                BillboardMode::Fixed => "Fixed",
                            })
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(definition.billboard_mode == BillboardMode::FaceCamera, "Face Camera").clicked() {
                                    definition.billboard_mode = BillboardMode::FaceCamera;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.billboard_mode == BillboardMode::FaceCameraY, "Face Camera (Y Lock)").clicked() {
                                    definition.billboard_mode = BillboardMode::FaceCameraY;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.billboard_mode == BillboardMode::Velocity, "Velocity").clicked() {
                                    definition.billboard_mode = BillboardMode::Velocity;
                                    changed = true;
                                }
                                if ui.selectable_label(definition.billboard_mode == BillboardMode::Fixed, "Fixed").clicked() {
                                    definition.billboard_mode = BillboardMode::Fixed;
                                    changed = true;
                                }
                            });
                    });
                });
            }
        }
    });

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
