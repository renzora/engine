//! Rendering component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::shared::{MaterialData, MeshNodeData, MeshPrimitiveType, Sprite2DData};
use crate::input::MaterialApplied;
use crate::ui::property_row;

use egui_phosphor::regular::{CUBE, IMAGE, PALETTE};

// ============================================================================
// Component Definitions
// ============================================================================

pub static MESH_RENDERER: ComponentDefinition = ComponentDefinition {
    type_id: "mesh_renderer",
    display_name: "Mesh Renderer",
    category: ComponentCategory::Rendering,
    icon: CUBE,
    priority: 0,
    add_fn: add_mesh_renderer,
    remove_fn: remove_mesh_renderer,
    has_fn: has_mesh_renderer,
    serialize_fn: serialize_mesh_renderer,
    deserialize_fn: deserialize_mesh_renderer,
    inspector_fn: inspect_mesh_renderer,
    conflicts_with: &["sprite_2d"],
    requires: &[],
};

pub static SPRITE_2D: ComponentDefinition = ComponentDefinition {
    type_id: "sprite_2d",
    display_name: "Sprite 2D",
    category: ComponentCategory::Rendering,
    icon: IMAGE,
    priority: 1,
    add_fn: add_sprite_2d,
    remove_fn: remove_sprite_2d,
    has_fn: has_sprite_2d,
    serialize_fn: serialize_sprite_2d,
    deserialize_fn: deserialize_sprite_2d,
    inspector_fn: inspect_sprite_2d,
    conflicts_with: &["mesh_renderer"],
    requires: &[],
};

pub static MATERIAL: ComponentDefinition = ComponentDefinition {
    type_id: "material",
    display_name: "Material",
    category: ComponentCategory::Rendering,
    icon: PALETTE,
    priority: 2,
    add_fn: add_material,
    remove_fn: remove_material,
    has_fn: has_material,
    serialize_fn: serialize_material,
    deserialize_fn: deserialize_material,
    inspector_fn: inspect_material,
    conflicts_with: &[],
    requires: &[],
};

/// Register all rendering components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&MESH_RENDERER);
    registry.register(&SPRITE_2D);
    registry.register(&MATERIAL);
}

// ============================================================================
// Mesh Renderer
// ============================================================================

fn add_mesh_renderer(
    commands: &mut Commands,
    entity: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.7, 0.6),
        ..default()
    });

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

fn has_mesh_renderer(world: &World, entity: Entity) -> bool {
    world.get::<Mesh3d>(entity).is_some()
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

    let mesh_type = match mesh_type_str {
        "Cube" => MeshPrimitiveType::Cube,
        "Sphere" => MeshPrimitiveType::Sphere,
        "Cylinder" => MeshPrimitiveType::Cylinder,
        "Plane" => MeshPrimitiveType::Plane,
        _ => MeshPrimitiveType::Cube,
    };

    let mesh = match mesh_type {
        MeshPrimitiveType::Cube => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        MeshPrimitiveType::Sphere => meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap()),
        MeshPrimitiveType::Cylinder => meshes.add(Cylinder::new(0.5, 2.0)),
        MeshPrimitiveType::Plane => meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(1.0))),
    };

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

    entity_commands.insert((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        MeshNodeData { mesh_type },
    ));
}

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
            ui.label("Mesh Type");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mesh_types = [
                    (MeshPrimitiveType::Cube, "Cube"),
                    (MeshPrimitiveType::Sphere, "Sphere"),
                    (MeshPrimitiveType::Cylinder, "Cylinder"),
                    (MeshPrimitiveType::Plane, "Plane"),
                ];

                let current_name = mesh_types
                    .iter()
                    .find(|(t, _)| *t == current_mesh_type)
                    .map(|(_, n)| *n)
                    .unwrap_or("Cube");

                egui::ComboBox::from_id_salt("mesh_type")
                    .selected_text(current_name)
                    .show_ui(ui, |ui| {
                        for (mesh_type, name) in mesh_types.iter() {
                            if ui
                                .selectable_value(&mut current_mesh_type.clone(), *mesh_type, *name)
                                .clicked()
                                && *mesh_type != current_mesh_type
                            {
                                // Update mesh
                                let new_mesh = match mesh_type {
                                    MeshPrimitiveType::Cube => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                                    MeshPrimitiveType::Sphere => {
                                        meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap())
                                    }
                                    MeshPrimitiveType::Cylinder => meshes.add(Cylinder::new(0.5, 2.0)),
                                    MeshPrimitiveType::Plane => {
                                        meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(1.0)))
                                    }
                                };
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
                    ui.label("Color");
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
                    ui.label("Roughness");
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
                    ui.label("Metallic");
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
// Sprite 2D
// ============================================================================

fn add_sprite_2d(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        Sprite {
            color: Color::WHITE,
            ..default()
        },
        Sprite2DData::default(),
    ));
}

fn remove_sprite_2d(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<Sprite>()
        .remove::<Sprite2DData>();
}

fn has_sprite_2d(world: &World, entity: Entity) -> bool {
    world.get::<Sprite>(entity).is_some()
}

fn serialize_sprite_2d(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<Sprite2DData>(entity)?;
    let sprite = world.get::<Sprite>(entity)?;
    let srgba = sprite.color.to_srgba();

    Some(json!({
        "texture_path": data.texture_path,
        "color": [srgba.red, srgba.green, srgba.blue, srgba.alpha],
        "flip_x": data.flip_x,
        "flip_y": data.flip_y,
        "anchor": [data.anchor.x, data.anchor.y]
    }))
}

fn deserialize_sprite_2d(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let texture_path = data
        .get("texture_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let color = data
        .get("color")
        .and_then(|c| c.as_array())
        .map(|arr| {
            Color::srgba(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Color::WHITE);

    let flip_x = data
        .get("flip_x")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let flip_y = data
        .get("flip_y")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let anchor = data
        .get("anchor")
        .and_then(|a| a.as_array())
        .map(|arr| {
            Vec2::new(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
            )
        })
        .unwrap_or(Vec2::new(0.5, 0.5));

    entity_commands.insert((
        Sprite {
            color,
            flip_x,
            flip_y,
            ..default()
        },
        Sprite2DData {
            texture_path,
            color: Vec4::new(
                color.to_srgba().red,
                color.to_srgba().green,
                color.to_srgba().blue,
                color.to_srgba().alpha,
            ),
            flip_x,
            flip_y,
            anchor,
        },
    ));
}

fn inspect_sprite_2d(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;

    // Get mutable references to components
    let Some(mut sprite) = world.get_mut::<Sprite>(entity) else {
        return false;
    };

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let srgba = sprite.color.to_srgba();
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (srgba.red * 255.0) as u8,
                    (srgba.green * 255.0) as u8,
                    (srgba.blue * 255.0) as u8,
                    (srgba.alpha * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    sprite.color = Color::srgba(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                        color.a() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    // Flip X
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Flip X");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut sprite.flip_x, "").changed() {
                    changed = true;
                }
            });
        });
    });

    // Flip Y
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Flip Y");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut sprite.flip_y, "").changed() {
                    changed = true;
                }
            });
        });
    });

    // Update Sprite2DData to match - copy values first, then update
    drop(sprite);

    // Get sprite values
    let sprite_values = world.get::<Sprite>(entity).map(|sprite| {
        let srgba = sprite.color.to_srgba();
        (sprite.flip_x, sprite.flip_y, Vec4::new(srgba.red, srgba.green, srgba.blue, srgba.alpha))
    });

    if let (Some((flip_x, flip_y, color)), Some(mut sprite_data)) =
        (sprite_values, world.get_mut::<Sprite2DData>(entity))
    {
        sprite_data.flip_x = flip_x;
        sprite_data.flip_y = flip_y;
        sprite_data.color = color;
    }

    changed
}

// ============================================================================
// Material (Blueprint Material)
// ============================================================================

fn add_material(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(MaterialData {
        material_path: None,
    });
}

fn remove_material(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<MaterialData>()
        .remove::<MaterialApplied>();
}

fn has_material(world: &World, entity: Entity) -> bool {
    world.get::<MaterialData>(entity).is_some()
}

fn serialize_material(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<MaterialData>(entity)?;
    Some(json!({
        "material_path": data.material_path
    }))
}

fn deserialize_material(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let material_path = data
        .get("material_path")
        .and_then(|v| {
            if v.is_null() {
                None
            } else {
                v.as_str().map(|s| s.to_string())
            }
        });

    entity_commands.insert(MaterialData { material_path });
}

fn inspect_material(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;

    // Get current material path
    let current_path = world
        .get::<MaterialData>(entity)
        .and_then(|d| d.material_path.clone());

    let display_path = current_path
        .as_ref()
        .map(|p| {
            // Show just the filename for brevity
            std::path::Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        })
        .unwrap_or("None");

    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Blueprint");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Show the current material path (read-only for now, drag-drop to change)
                ui.label(display_path);
            });
        });
    });

    // Show applied status
    let is_applied = world.get::<MaterialApplied>(entity).is_some();
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Status");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if current_path.is_some() {
                    if is_applied {
                        ui.colored_label(egui::Color32::from_rgb(100, 200, 100), "Applied");
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(200, 200, 100), "Pending...");
                    }
                } else {
                    ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "No material");
                }
            });
        });
    });

    // Clear button
    if current_path.is_some() {
        property_row(ui, 2, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Clear Material").clicked() {
                    if let Some(mut data) = world.get_mut::<MaterialData>(entity) {
                        data.material_path = None;
                    }
                    // Remove the applied marker so the system knows to update
                    // (This needs commands, but we're in world access - mark changed and handle elsewhere)
                    changed = true;
                }
            });
        });
    }

    // Hint text
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Drag a .material_bp file here to apply")
            .small()
            .color(egui::Color32::from_rgb(120, 120, 130)),
    );

    changed
}
