//! Terrain component definitions

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::terrain::{TerrainData, TerrainChunkData, TerrainChunkOf, generate_chunk_mesh};
use crate::core::{EditorEntity, SceneNode};
use crate::ui::property_row;

use egui_phosphor::regular::MOUNTAINS;

/// Default terrain material path
pub const DEFAULT_TERRAIN_MATERIAL: &str = "materials/terrain_default.mat";

/// Marker component for entities that need their terrain material loaded
#[derive(Component, Default)]
pub struct NeedsTerrainMaterial;

// ============================================================================
// Custom Add/Remove (terrain needs to spawn chunk children)
// ============================================================================

fn add_terrain(
    commands: &mut Commands,
    entity: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // Default terrain configuration - 4x4 chunks
    let terrain_data = TerrainData {
        chunks_x: 4,
        chunks_z: 4,
        chunk_size: 64.0,
        chunk_resolution: 33,
        max_height: 50.0,
        min_height: -10.0,
    };

    // Create terrain material
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.45, 0.25),
        perceptual_roughness: 0.95,
        ..default()
    });

    let initial_height = 0.2;

    // Add terrain data to the root entity
    commands.entity(entity).insert(terrain_data.clone());

    // Spawn chunk entities as children
    for cz in 0..terrain_data.chunks_z {
        for cx in 0..terrain_data.chunks_x {
            let mut chunk_data = TerrainChunkData::new(
                cx,
                cz,
                terrain_data.chunk_resolution,
                initial_height,
            );
            chunk_data.dirty = false;

            let mesh = generate_chunk_mesh(&terrain_data, &chunk_data);
            let mesh_handle = meshes.add(mesh);

            let origin = terrain_data.chunk_world_origin(cx, cz);

            // Note: RaytracingMesh3d is managed by sync_rendering_settings based on Solari state
            commands.spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(origin),
                Visibility::default(),
                EditorEntity {
                    name: format!("Chunk_{}_{}", cx, cz),
                    tag: String::new(),
                    visible: true,
                    locked: false,
                },
                SceneNode,
                chunk_data,
                TerrainChunkOf(entity),
                ChildOf(entity),
                NeedsTerrainMaterial,
            ));
        }
    }
}

fn remove_terrain(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<TerrainData>();
    // Note: Child chunks will be removed automatically when parent is despawned
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_terrain(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<TerrainData>(entity) else {
        return false;
    };
    let mut changed = false;

    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Chunks X");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.chunks_x).speed(1.0).range(1..=64)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Chunks Z");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.chunks_z).speed(1.0).range(1..=64)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Chunk Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.chunk_size).speed(1.0).range(1.0..=256.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Resolution");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.chunk_resolution).speed(1.0).range(3..=129)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Max Height");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.max_height).speed(1.0).range(-1000.0..=1000.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    property_row(ui, 5, |ui| {
        ui.horizontal(|ui| {
            ui.label("Min Height");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.min_height).speed(1.0).range(-1000.0..=1000.0)).changed() {
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

/// Register all terrain components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(TerrainData {
        type_id: "terrain",
        display_name: "Terrain",
        category: ComponentCategory::Rendering,
        icon: MOUNTAINS,
        priority: 110,
        custom_inspector: inspect_terrain,
        custom_add: add_terrain,
        custom_remove: remove_terrain,
    }));
}
