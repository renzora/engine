//! Mesh renderer component definition

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::component_system::{MeshNodeData, MeshPrimitiveType};
use crate::spawn::meshes::create_mesh_for_type;
use crate::ui::property_row;

use egui_phosphor::regular::CUBE;

// ============================================================================
// Custom Add/Remove/Serialize/Deserialize
// ============================================================================

fn add_mesh_renderer(
    commands: &mut Commands,
    entity: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Cube);
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.7, 0.6),
        ..default()
    });

    // Note: RaytracingMesh3d is managed by sync_rendering_settings based on Solari state
    commands.entity(entity).insert((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        MeshNodeData {
            mesh_type: MeshPrimitiveType::Cube,
        },
    ));
}

fn remove_mesh_renderer(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<Mesh3d>()
        .remove::<MeshMaterial3d<StandardMaterial>>()
        .remove::<MeshNodeData>();
}

fn serialize_mesh_renderer(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let mesh_data = world.get::<MeshNodeData>(entity)?;

    // Get material data if available
    let mut color = [0.8f32, 0.7, 0.6];
    let mut roughness = 0.5f32;
    let mut metallic = 0.0f32;

    if let Some(material_handle) = world.get::<MeshMaterial3d<StandardMaterial>>(entity) {
        if let Some(materials) = world.get_resource::<Assets<StandardMaterial>>() {
            if let Some(mat) = materials.get(&material_handle.0) {
                let srgba = mat.base_color.to_srgba();
                color = [srgba.red, srgba.green, srgba.blue];
                roughness = mat.perceptual_roughness;
                metallic = mat.metallic;
            }
        }
    }

    Some(json!({
        "mesh_type": format!("{:?}", mesh_data.mesh_type),
        "color": color,
        "roughness": roughness,
        "metallic": metallic
    }))
}

fn deserialize_mesh_renderer(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mesh_type_str = data
        .get("mesh_type")
        .and_then(|v| v.as_str())
        .unwrap_or("Cube");

    let mesh_type = MeshPrimitiveType::all()
        .iter()
        .find(|t| format!("{:?}", t) == mesh_type_str)
        .copied()
        .unwrap_or(MeshPrimitiveType::Cube);

    let mesh = create_mesh_for_type(meshes, mesh_type);

    let color = data
        .get("color")
        .and_then(|c| c.as_array())
        .map(|arr| {
            Color::srgb(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(0.8) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.7) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.6) as f32,
            )
        })
        .unwrap_or(Color::srgb(0.8, 0.7, 0.6));

    let roughness = data
        .get("roughness")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5) as f32;

    let metallic = data
        .get("metallic")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;

    let material = materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: roughness,
        metallic,
        ..default()
    });

    // Note: RaytracingMesh3d is managed by sync_rendering_settings based on Solari state
    entity_commands.insert((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        MeshNodeData { mesh_type },
    ));
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_mesh_renderer(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;

    // Get current mesh data
    let current_mesh_type = world
        .get::<MeshNodeData>(entity)
        .map(|d| d.mesh_type)
        .unwrap_or(MeshPrimitiveType::Cube);

    // Mesh type selector
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label(crate::locale::t("comp.mesh.type"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let all_types = MeshPrimitiveType::all();

                let current_name = current_mesh_type.display_name();

                egui::ComboBox::from_id_salt("mesh_type")
                    .selected_text(current_name)
                    .show_ui(ui, |ui| {
                        for mesh_type in all_types {
                            if ui
                                .selectable_value(&mut current_mesh_type.clone(), *mesh_type, mesh_type.display_name())
                                .clicked()
                                && *mesh_type != current_mesh_type
                            {
                                // Update mesh
                                let new_mesh = create_mesh_for_type(meshes, *mesh_type);
                                if let Some(mut mesh_handle) = world.get_mut::<Mesh3d>(entity) {
                                    mesh_handle.0 = new_mesh;
                                }
                                if let Some(mut mesh_data) = world.get_mut::<MeshNodeData>(entity) {
                                    mesh_data.mesh_type = *mesh_type;
                                }
                                changed = true;
                            }
                        }
                    });
            });
        });
    });

    // Material properties
    if let Some(material_handle) = world.get::<MeshMaterial3d<StandardMaterial>>(entity) {
        let material_id = material_handle.0.id();
        if let Some(mat) = materials.get_mut(material_id) {
            // Color
            property_row(ui, 1, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::locale::t("common.color"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let srgba = mat.base_color.to_srgba();
                        let mut color = egui::Color32::from_rgb(
                            (srgba.red * 255.0) as u8,
                            (srgba.green * 255.0) as u8,
                            (srgba.blue * 255.0) as u8,
                        );
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            mat.base_color = Color::srgb(
                                color.r() as f32 / 255.0,
                                color.g() as f32 / 255.0,
                                color.b() as f32 / 255.0,
                            );
                            changed = true;
                        }
                    });
                });
            });

            // Roughness
            property_row(ui, 2, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::locale::t("comp.mesh.roughness"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::DragValue::new(&mut mat.perceptual_roughness)
                                    .speed(0.01)
                                    .range(0.0..=1.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                });
            });

            // Metallic
            property_row(ui, 3, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::locale::t("comp.mesh.metallic"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::DragValue::new(&mut mat.metallic)
                                    .speed(0.01)
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
    }

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(MeshNodeData {
        type_id: "mesh_renderer",
        display_name: "Mesh Renderer",
        category: ComponentCategory::Rendering,
        icon: CUBE,
        priority: 0,
        conflicts_with: ["sprite_2d"],
        custom_inspector: inspect_mesh_renderer,
        custom_add: add_mesh_renderer,
        custom_remove: remove_mesh_renderer,
        custom_serialize: serialize_mesh_renderer,
        custom_deserialize: deserialize_mesh_renderer,
    }));
}
