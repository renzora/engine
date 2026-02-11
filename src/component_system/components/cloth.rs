//! Cloth simulation component definition

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{
    ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType,
};
use crate::register_component;
use crate::ui::property_row;

use egui_phosphor::regular::WIND;

use bevy_silk::prelude::*;

// ============================================================================
// Component Definition
// ============================================================================

/// Serializable cloth simulation data that drives bevy_silk's ClothBuilder.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ClothData {
    /// Vertex IDs to pin to the entity's transform
    pub pinned_vertex_ids: Vec<usize>,
    /// How sticks are generated from mesh topology
    pub stick_generation: ClothStickGeneration,
    /// Stick length mode
    pub stick_length: ClothStickLength,
    /// Stick behaviour mode
    pub stick_mode: ClothStickMode,
    /// Normal computation mode
    pub normals: ClothNormals,
    /// Enable collision detection with physics colliders
    pub collisions_enabled: bool,
    /// Collision offset to prevent clipping
    pub collision_offset: f32,
    /// How much collider velocity is transferred to cloth
    pub collision_velocity_coeff: f32,
    /// Override gravity (if None, uses global ClothConfig)
    pub custom_gravity: Option<[f32; 3]>,
    /// Override friction (if None, uses global ClothConfig)
    pub custom_friction: Option<f32>,
    /// Override sticks computation depth (if None, uses global)
    pub custom_sticks_depth: Option<u8>,
}

/// Serializable stick generation mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum ClothStickGeneration {
    #[default]
    Quads,
    Triangles,
}

/// Serializable stick length mode
#[derive(Clone, Copy, Debug, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub enum ClothStickLength {
    #[default]
    Auto,
    Fixed(f32),
    Offset(f32),
    Coefficient(f32),
}

/// Serializable stick mode
#[derive(Clone, Copy, Debug, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub enum ClothStickMode {
    #[default]
    Fixed,
    Spring { min_percent: f32, max_percent: f32 },
}

/// Serializable normal computation mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum ClothNormals {
    None,
    #[default]
    SmoothNormals,
    FlatNormals,
}

impl Default for ClothData {
    fn default() -> Self {
        Self {
            pinned_vertex_ids: Vec::new(),
            stick_generation: ClothStickGeneration::default(),
            stick_length: ClothStickLength::default(),
            stick_mode: ClothStickMode::default(),
            normals: ClothNormals::default(),
            collisions_enabled: true,
            collision_offset: 0.25,
            collision_velocity_coeff: 1.0,
            custom_gravity: None,
            custom_friction: None,
            custom_sticks_depth: None,
        }
    }
}

// ============================================================================
// Conversion helpers
// ============================================================================

impl ClothData {
    fn to_cloth_builder(&self) -> ClothBuilder {
        let mut builder = ClothBuilder::new()
            .with_pinned_vertex_ids(self.pinned_vertex_ids.iter().copied())
            .with_stick_generation(match self.stick_generation {
                ClothStickGeneration::Quads => StickGeneration::Quads,
                ClothStickGeneration::Triangles => StickGeneration::Triangles,
            })
            .with_stick_length(match self.stick_length {
                ClothStickLength::Auto => StickLen::Auto,
                ClothStickLength::Fixed(v) => StickLen::Fixed(v),
                ClothStickLength::Offset(v) => StickLen::Offset(v),
                ClothStickLength::Coefficient(v) => StickLen::Coefficient(v),
            })
            .with_stick_mode(match self.stick_mode {
                ClothStickMode::Fixed => StickMode::Fixed,
                ClothStickMode::Spring {
                    min_percent,
                    max_percent,
                } => StickMode::Spring {
                    min_percent,
                    max_percent,
                },
            });
        builder.normals_computing = match self.normals {
            ClothNormals::None => NormalComputing::None,
            ClothNormals::SmoothNormals => NormalComputing::SmoothNormals,
            ClothNormals::FlatNormals => NormalComputing::FlatNormals,
        };
        builder
    }

    fn to_cloth_config(&self) -> Option<ClothConfig> {
        if self.custom_gravity.is_none()
            && self.custom_friction.is_none()
            && self.custom_sticks_depth.is_none()
        {
            return None;
        }
        let defaults = ClothConfig::default();
        Some(ClothConfig {
            gravity: self
                .custom_gravity
                .map(Vec3::from)
                .unwrap_or(defaults.gravity),
            friction: self.custom_friction.unwrap_or(defaults.friction),
            sticks_computation_depth: self
                .custom_sticks_depth
                .unwrap_or(defaults.sticks_computation_depth),
            acceleration_smoothing: defaults.acceleration_smoothing,
        })
    }
}

// ============================================================================
// Custom Add / Remove / Deserialize
// ============================================================================

fn add_cloth(
    commands: &mut Commands,
    entity: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // Generate a subdivided cloth-ready plane mesh (20x20 vertices)
    // bevy_silk needs many vertices — a 4-vertex plane won't simulate
    // step_y uses +Z so triangle winding faces up (+Y normal, not backface-culled)
    let cloth_mesh = rectangle_mesh(
        (20, 20),
        (Vec3::X * 0.1, Vec3::Z * 0.1),  // ~2x2 unit plane
        Vec3::Y,
    );
    let mesh_handle = meshes.add(cloth_mesh);
    // Double-sided material so cloth is visible from both sides
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.7, 0.6),
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    // Pin the top row (first 20 vertex IDs) so the cloth hangs
    let mut data = ClothData::default();
    data.pinned_vertex_ids = (0..20).collect();
    let builder = data.to_cloth_builder();
    let mut entity_commands = commands.entity(entity);
    entity_commands.insert((data, builder, Mesh3d(mesh_handle), MeshMaterial3d(material)));
    entity_commands.insert(ClothCollider::default());
}

fn remove_cloth(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<ClothData>()
        .remove::<ClothBuilder>()
        .remove::<ClothCollider>()
        .remove::<ClothConfig>()
        .remove::<bevy_silk::components::cloth::Cloth>()
        .remove::<bevy_silk::components::cloth_rendering::ClothRendering>();
}

fn deserialize_cloth(
    entity_commands: &mut EntityCommands,
    json: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(data) = serde_json::from_value::<ClothData>(json.clone()) {
        let builder = data.to_cloth_builder();
        let config = data.to_cloth_config();
        let collider = if data.collisions_enabled {
            Some(ClothCollider {
                offset: data.collision_offset,
                velocity_coefficient: data.collision_velocity_coeff,
                dampen_others: None,
            })
        } else {
            None
        };
        entity_commands.insert((data, builder));
        if let Some(collider) = collider {
            entity_commands.insert(collider);
        }
        if let Some(config) = config {
            entity_commands.insert(config);
        }
    }
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_cloth(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<ClothData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Pinned vertices
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Pinned Vertices");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{} pinned", data.pinned_vertex_ids.len()));
            });
        });
    });

    // Pinned vertex editor
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Pin Range");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Clear").clicked() {
                    data.pinned_vertex_ids.clear();
                    changed = true;
                }
            });
        });
    });

    // Quick pin range input
    ui.horizontal(|ui| {
        ui.label("  Pin IDs 0..");
        let mut end = data
            .pinned_vertex_ids
            .iter()
            .copied()
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);
        if ui
            .add(egui::DragValue::new(&mut end).speed(1.0).range(0..=10000))
            .changed()
        {
            data.pinned_vertex_ids = (0..end).collect();
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Stick Generation
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Stick Generation");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let options = [
                    (ClothStickGeneration::Quads, "Quads"),
                    (ClothStickGeneration::Triangles, "Triangles"),
                ];
                let current = options
                    .iter()
                    .find(|(v, _)| *v == data.stick_generation)
                    .map(|(_, n)| *n)
                    .unwrap_or("Quads");

                egui::ComboBox::from_id_salt("cloth_stick_gen")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (val, name) in &options {
                            if ui
                                .selectable_value(&mut data.stick_generation, *val, *name)
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    });
            });
        });
    });

    // Stick Length
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Stick Length");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mode_name = match data.stick_length {
                    ClothStickLength::Auto => "Auto",
                    ClothStickLength::Fixed(_) => "Fixed",
                    ClothStickLength::Offset(_) => "Offset",
                    ClothStickLength::Coefficient(_) => "Coefficient",
                };

                egui::ComboBox::from_id_salt("cloth_stick_len")
                    .selected_text(mode_name)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                matches!(data.stick_length, ClothStickLength::Auto),
                                "Auto",
                            )
                            .clicked()
                        {
                            data.stick_length = ClothStickLength::Auto;
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                matches!(data.stick_length, ClothStickLength::Fixed(_)),
                                "Fixed",
                            )
                            .clicked()
                        {
                            data.stick_length = ClothStickLength::Fixed(1.0);
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                matches!(data.stick_length, ClothStickLength::Offset(_)),
                                "Offset",
                            )
                            .clicked()
                        {
                            data.stick_length = ClothStickLength::Offset(0.0);
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                matches!(data.stick_length, ClothStickLength::Coefficient(_)),
                                "Coefficient",
                            )
                            .clicked()
                        {
                            data.stick_length = ClothStickLength::Coefficient(1.0);
                            changed = true;
                        }
                    });
            });
        });
    });

    // Stick length value (if applicable)
    match &mut data.stick_length {
        ClothStickLength::Fixed(v) | ClothStickLength::Offset(v) | ClothStickLength::Coefficient(v) => {
            property_row(ui, 4, |ui| {
                ui.horizontal(|ui| {
                    ui.label("  Value");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::DragValue::new(v).speed(0.01).range(0.001..=100.0))
                            .changed()
                        {
                            changed = true;
                        }
                    });
                });
            });
        }
        _ => {}
    }

    // Stick Mode
    property_row(ui, 5, |ui| {
        ui.horizontal(|ui| {
            ui.label("Stick Mode");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mode_name = match data.stick_mode {
                    ClothStickMode::Fixed => "Fixed",
                    ClothStickMode::Spring { .. } => "Spring",
                };

                egui::ComboBox::from_id_salt("cloth_stick_mode")
                    .selected_text(mode_name)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                matches!(data.stick_mode, ClothStickMode::Fixed),
                                "Fixed",
                            )
                            .clicked()
                        {
                            data.stick_mode = ClothStickMode::Fixed;
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                matches!(data.stick_mode, ClothStickMode::Spring { .. }),
                                "Spring",
                            )
                            .clicked()
                        {
                            data.stick_mode = ClothStickMode::Spring {
                                min_percent: 0.8,
                                max_percent: 1.2,
                            };
                            changed = true;
                        }
                    });
            });
        });
    });

    // Spring params
    if let ClothStickMode::Spring {
        min_percent,
        max_percent,
    } = &mut data.stick_mode
    {
        property_row(ui, 6, |ui| {
            ui.horizontal(|ui| {
                ui.label("  Min %");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::DragValue::new(min_percent)
                                .speed(0.01)
                                .range(0.0..=5.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
        property_row(ui, 7, |ui| {
            ui.horizontal(|ui| {
                ui.label("  Max %");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::DragValue::new(max_percent)
                                .speed(0.01)
                                .range(0.0..=5.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
    }

    // Normals
    property_row(ui, 8, |ui| {
        ui.horizontal(|ui| {
            ui.label("Normals");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let options = [
                    (ClothNormals::SmoothNormals, "Smooth"),
                    (ClothNormals::FlatNormals, "Flat"),
                    (ClothNormals::None, "None"),
                ];
                let current = options
                    .iter()
                    .find(|(v, _)| *v == data.normals)
                    .map(|(_, n)| *n)
                    .unwrap_or("Smooth");

                egui::ComboBox::from_id_salt("cloth_normals")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (val, name) in &options {
                            if ui
                                .selectable_value(&mut data.normals, *val, *name)
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    });
            });
        });
    });

    ui.add_space(4.0);
    ui.label("Collisions");

    // Collisions enabled
    property_row(ui, 9, |ui| {
        ui.horizontal(|ui| {
            ui.label("Enable Collisions");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.collisions_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    if data.collisions_enabled {
        property_row(ui, 10, |ui| {
            ui.horizontal(|ui| {
                ui.label("  Offset");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut data.collision_offset)
                                .speed(0.01)
                                .range(0.0..=10.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });

        property_row(ui, 11, |ui| {
            ui.horizontal(|ui| {
                ui.label("  Velocity Coeff");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut data.collision_velocity_coeff)
                                .speed(0.01)
                                .range(0.0..=5.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
    }

    // Per-entity physics overrides
    ui.add_space(4.0);
    ui.label("Physics Overrides");

    // Gravity override
    property_row(ui, 12, |ui| {
        ui.horizontal(|ui| {
            let mut has_gravity = data.custom_gravity.is_some();
            if ui.checkbox(&mut has_gravity, "Custom Gravity").changed() {
                data.custom_gravity = if has_gravity {
                    Some([0.0, -9.81, 0.0])
                } else {
                    None
                };
                changed = true;
            }
        });
    });

    if let Some(gravity) = &mut data.custom_gravity {
        property_row(ui, 13, |ui| {
            ui.horizontal(|ui| {
                ui.label("  ");
                for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(*label);
                    if ui
                        .add(
                            egui::DragValue::new(&mut gravity[i])
                                .speed(0.1)
                                .range(-100.0..=100.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                }
            });
        });
    }

    // Friction override
    property_row(ui, 14, |ui| {
        ui.horizontal(|ui| {
            let mut has_friction = data.custom_friction.is_some();
            if ui.checkbox(&mut has_friction, "Custom Friction").changed() {
                data.custom_friction = if has_friction { Some(0.01) } else { None };
                changed = true;
            }
        });
    });

    if let Some(friction) = &mut data.custom_friction {
        property_row(ui, 15, |ui| {
            ui.horizontal(|ui| {
                ui.label("  Friction");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::DragValue::new(friction)
                                .speed(0.001)
                                .range(0.0..=1.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
    }

    // Sticks depth override
    property_row(ui, 16, |ui| {
        ui.horizontal(|ui| {
            let mut has_depth = data.custom_sticks_depth.is_some();
            if ui.checkbox(&mut has_depth, "Custom Sticks Depth").changed() {
                data.custom_sticks_depth = if has_depth { Some(5) } else { None };
                changed = true;
            }
        });
    });

    if let Some(depth) = &mut data.custom_sticks_depth {
        property_row(ui, 17, |ui| {
            ui.horizontal(|ui| {
                ui.label("  Depth");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut depth_u32 = *depth as u32;
                    if ui
                        .add(egui::DragValue::new(&mut depth_u32).speed(1.0).range(1..=50))
                        .changed()
                    {
                        *depth = depth_u32 as u8;
                        changed = true;
                    }
                });
            });
        });
    }

    changed
}

// ============================================================================
// Script Properties
// ============================================================================

fn cloth_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("collisions_enabled", PropertyValueType::Bool),
        ("collision_offset", PropertyValueType::Float),
        ("collision_velocity_coeff", PropertyValueType::Float),
    ]
}

fn cloth_get_props(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<ClothData>(entity) else {
        return vec![];
    };
    vec![
        (
            "collisions_enabled",
            PropertyValue::Bool(data.collisions_enabled),
        ),
        (
            "collision_offset",
            PropertyValue::Float(data.collision_offset),
        ),
        (
            "collision_velocity_coeff",
            PropertyValue::Float(data.collision_velocity_coeff),
        ),
    ]
}

fn cloth_set_prop(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<ClothData>(entity) else {
        return false;
    };
    match prop {
        "collisions_enabled" => {
            if let PropertyValue::Bool(v) = val {
                data.collisions_enabled = *v;
                true
            } else {
                false
            }
        }
        "collision_offset" => {
            if let PropertyValue::Float(v) = val {
                data.collision_offset = *v;
                true
            } else {
                false
            }
        }
        "collision_velocity_coeff" => {
            if let PropertyValue::Float(v) = val {
                data.collision_velocity_coeff = *v;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

// ============================================================================
// Sync System — rebuilds bevy_silk components when ClothData changes
// ============================================================================

pub fn sync_cloth_data(
    mut commands: Commands,
    query: Query<(Entity, Ref<ClothData>)>,
    mut prev_state: Local<bevy::platform::collections::HashMap<Entity, ClothData>>,
) {
    for (entity, data) in query.iter() {
        // Only process if Bevy's change detection fired
        if !data.is_changed() {
            continue;
        }

        // On first add, always set up
        if data.is_added() {
            prev_state.insert(entity, data.clone());
            rebuild_cloth_entity(&mut commands, entity, &data);
            continue;
        }

        // Compare against previous state — skip if the inspector just touched
        // it via get_mut without the user actually editing anything
        if let Some(prev) = prev_state.get(&entity) {
            if *prev == *data {
                continue;
            }
        }

        prev_state.insert(entity, data.clone());
        rebuild_cloth_entity(&mut commands, entity, &data);
    }
}

fn rebuild_cloth_entity(commands: &mut Commands, entity: Entity, data: &ClothData) {
    let builder = data.to_cloth_builder();
    let config = data.to_cloth_config();

    let mut ecmds = commands.entity(entity);

    // Re-insert builder (triggers bevy_silk re-init via Added<ClothBuilder>)
    ecmds.insert(builder);

    // Remove runtime cloth state so bevy_silk reinitializes from the new ClothBuilder
    ecmds
        .remove::<bevy_silk::components::cloth::Cloth>()
        .remove::<bevy_silk::components::cloth_rendering::ClothRendering>();

    // Sync collider
    if data.collisions_enabled {
        ecmds.insert(ClothCollider {
            offset: data.collision_offset,
            velocity_coefficient: data.collision_velocity_coeff,
            dampen_others: None,
        });
    } else {
        ecmds.remove::<ClothCollider>();
    }

    // Sync per-entity config override
    if let Some(cfg) = config {
        ecmds.insert(cfg);
    } else {
        ecmds.remove::<ClothConfig>();
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(ClothData {
        type_id: "cloth",
        display_name: "Cloth",
        category: ComponentCategory::Physics,
        icon: WIND,
        priority: 1,
        custom_inspector: inspect_cloth,
        custom_add: add_cloth,
        custom_remove: remove_cloth,
        custom_deserialize: deserialize_cloth,
        custom_script_properties: cloth_get_props,
        custom_script_set: cloth_set_prop,
        custom_script_meta: cloth_property_meta,
    }));
}
