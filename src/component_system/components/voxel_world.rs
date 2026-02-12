use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::ui::property_row;

use egui_phosphor::regular::CUBE;

// ============================================================================
// Component Definition
// ============================================================================

/// Noise algorithm for procedural voxel terrain generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum VoxelNoiseType {
    #[default]
    Perlin,
    Simplex,
}

/// Serializable voxel world configuration data.
/// Drives bevy_voxel_world's RenzoraVoxelConfig via a sync system.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct VoxelWorldData {
    /// Whether the voxel world is active
    pub enabled: bool,
    /// Chunk spawn radius around the camera
    pub spawning_distance: u32,
    /// Minimum chunk despawn radius
    pub min_despawn_distance: u32,
    /// Max chunks queued for spawn per frame
    pub max_spawn_per_frame: usize,
    /// Procedural generation seed
    pub seed: u64,
    /// Noise algorithm
    pub noise_type: VoxelNoiseType,
    /// Noise frequency (feature scale)
    pub noise_frequency: f32,
    /// FBM octave count
    pub noise_octaves: u32,
    /// Terrain height multiplier in voxels
    pub noise_amplitude: f32,
    /// Y offset for terrain baseline
    pub base_height: i32,
    /// Enable 3D noise cave generation
    pub caves_enabled: bool,
    /// Cave density threshold (0.0-1.0)
    pub cave_threshold: f32,
    /// Debug chunk boundary visualization
    pub debug_draw_chunks: bool,
}

impl Default for VoxelWorldData {
    fn default() -> Self {
        Self {
            enabled: true,
            spawning_distance: 10,
            min_despawn_distance: 1,
            max_spawn_per_frame: 10000,
            seed: 1234,
            noise_type: VoxelNoiseType::default(),
            noise_frequency: 0.02,
            noise_octaves: 5,
            noise_amplitude: 50.0,
            base_height: 0,
            caves_enabled: false,
            cave_threshold: 0.3,
            debug_draw_chunks: false,
        }
    }
}

// ============================================================================
// Custom Add / Remove / Deserialize
// ============================================================================

fn add_voxel_world(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(VoxelWorldData::default());
}

fn remove_voxel_world(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<VoxelWorldData>();
}

fn deserialize_voxel_world(
    entity_commands: &mut EntityCommands,
    json: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(data) = serde_json::from_value::<VoxelWorldData>(json.clone()) {
        entity_commands.insert(data);
    }
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_voxel_world(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<VoxelWorldData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Enabled toggle
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Enabled");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    ui.add_space(4.0);
    ui.label("Generation");

    // Seed
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Seed");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut seed_i64 = data.seed as i64;
                if ui
                    .add(egui::DragValue::new(&mut seed_i64).speed(1.0))
                    .changed()
                {
                    data.seed = seed_i64 as u64;
                    changed = true;
                }
            });
        });
    });

    // Noise Type
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Noise Type");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let options = [
                    (VoxelNoiseType::Perlin, "Perlin"),
                    (VoxelNoiseType::Simplex, "Simplex"),
                ];
                let current = options
                    .iter()
                    .find(|(v, _)| *v == data.noise_type)
                    .map(|(_, n)| *n)
                    .unwrap_or("Perlin");

                egui::ComboBox::from_id_salt("voxel_noise_type")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (val, name) in &options {
                            if ui
                                .selectable_value(&mut data.noise_type, *val, *name)
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    });
            });
        });
    });

    // Frequency
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Frequency");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.noise_frequency)
                            .speed(0.001)
                            .range(0.001..=1.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Octaves
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Octaves");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.noise_octaves)
                            .speed(1.0)
                            .range(1..=6),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Amplitude
    property_row(ui, 5, |ui| {
        ui.horizontal(|ui| {
            ui.label("Amplitude");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.noise_amplitude)
                            .speed(1.0)
                            .range(1.0..=500.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Base Height
    property_row(ui, 6, |ui| {
        ui.horizontal(|ui| {
            ui.label("Base Height");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.base_height)
                            .speed(1.0)
                            .range(-500..=500),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    ui.add_space(4.0);
    ui.label("Spawning");

    // Spawning Distance
    property_row(ui, 7, |ui| {
        ui.horizontal(|ui| {
            ui.label("Spawn Distance");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.spawning_distance)
                            .speed(1.0)
                            .range(1..=100),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Min Despawn Distance
    property_row(ui, 8, |ui| {
        ui.horizontal(|ui| {
            ui.label("Min Despawn Dist");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.min_despawn_distance)
                            .speed(1.0)
                            .range(1..=100),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Max Spawn Per Frame
    property_row(ui, 9, |ui| {
        ui.horizontal(|ui| {
            ui.label("Max Spawn/Frame");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.max_spawn_per_frame)
                            .speed(100.0)
                            .range(1..=100000),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    ui.add_space(4.0);
    ui.label("Options");

    // Caves
    property_row(ui, 10, |ui| {
        ui.horizontal(|ui| {
            ui.label("Caves");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.caves_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    if data.caves_enabled {
        property_row(ui, 11, |ui| {
            ui.horizontal(|ui| {
                ui.label("  Cave Threshold");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut data.cave_threshold)
                                .speed(0.01)
                                .range(0.01..=1.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
    }

    // Debug Draw
    property_row(ui, 12, |ui| {
        ui.horizontal(|ui| {
            ui.label("Debug Chunks");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.debug_draw_chunks, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(VoxelWorldData {
        type_id: "voxel_world",
        display_name: "Voxel World",
        category: ComponentCategory::Rendering,
        icon: CUBE,
        priority: 111,
        custom_inspector: inspect_voxel_world,
        custom_add: add_voxel_world,
        custom_remove: remove_voxel_world,
        custom_deserialize: deserialize_voxel_world,
    }));
}
