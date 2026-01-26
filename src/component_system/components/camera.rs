//! Camera component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::shared::{Camera2DData, CameraNodeData, CameraRigData};
use crate::ui::property_row;

use egui_phosphor::regular::{VIDEO_CAMERA, APERTURE};

// ============================================================================
// Component Definitions
// ============================================================================

pub static CAMERA_3D: ComponentDefinition = ComponentDefinition {
    type_id: "camera_3d",
    display_name: "Camera 3D",
    category: ComponentCategory::Camera,
    icon: VIDEO_CAMERA,
    priority: 0,
    add_fn: add_camera_3d,
    remove_fn: remove_camera_3d,
    has_fn: has_camera_3d,
    serialize_fn: serialize_camera_3d,
    deserialize_fn: deserialize_camera_3d,
    inspector_fn: inspect_camera_3d,
    conflicts_with: &["camera_2d", "camera_rig"],
    requires: &[],
};

pub static CAMERA_2D: ComponentDefinition = ComponentDefinition {
    type_id: "camera_2d",
    display_name: "Camera 2D",
    category: ComponentCategory::Camera,
    icon: APERTURE,
    priority: 1,
    add_fn: add_camera_2d,
    remove_fn: remove_camera_2d,
    has_fn: has_camera_2d,
    serialize_fn: serialize_camera_2d,
    deserialize_fn: deserialize_camera_2d,
    inspector_fn: inspect_camera_2d,
    conflicts_with: &["camera_3d", "camera_rig"],
    requires: &[],
};

pub static CAMERA_RIG: ComponentDefinition = ComponentDefinition {
    type_id: "camera_rig",
    display_name: "Camera Rig",
    category: ComponentCategory::Camera,
    icon: VIDEO_CAMERA,
    priority: 2,
    add_fn: add_camera_rig,
    remove_fn: remove_camera_rig,
    has_fn: has_camera_rig,
    serialize_fn: serialize_camera_rig,
    deserialize_fn: deserialize_camera_rig,
    inspector_fn: inspect_camera_rig,
    conflicts_with: &["camera_3d", "camera_2d"],
    requires: &[],
};

/// Register all camera components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&CAMERA_3D);
    registry.register(&CAMERA_2D);
    registry.register(&CAMERA_RIG);
}

// ============================================================================
// Camera 3D
// ============================================================================

fn add_camera_3d(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        Camera3d::default(),
        CameraNodeData::default(),
    ));
}

fn remove_camera_3d(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<Camera3d>()
        .remove::<Camera>()
        .remove::<CameraNodeData>();
}

fn has_camera_3d(world: &World, entity: Entity) -> bool {
    world.get::<Camera3d>(entity).is_some() && world.get::<CameraNodeData>(entity).is_some()
}

fn serialize_camera_3d(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<CameraNodeData>(entity)?;
    Some(json!({
        "fov": data.fov,
        "is_default_camera": data.is_default_camera
    }))
}

fn deserialize_camera_3d(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let fov = data.get("fov").and_then(|v| v.as_f64()).unwrap_or(45.0) as f32;

    let is_default_camera = data
        .get("is_default_camera")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    entity_commands.insert((
        Camera3d::default(),
        CameraNodeData {
            fov,
            is_default_camera,
        },
    ));
}

fn inspect_camera_3d(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<CameraNodeData>(entity) else {
        return false;
    };
    let mut changed = false;

    // FOV
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("FOV");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.fov)
                            .speed(0.5)
                            .range(10.0..=170.0)
                            .suffix("\u{00b0}"),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Is Default Camera
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Default Camera");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.is_default_camera, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Camera 2D
// ============================================================================

fn add_camera_2d(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        Camera2d,
        Camera2DData::default(),
    ));
}

fn remove_camera_2d(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<Camera2d>()
        .remove::<Camera>()
        .remove::<Camera2DData>();
}

fn has_camera_2d(world: &World, entity: Entity) -> bool {
    world.get::<Camera2d>(entity).is_some()
}

fn serialize_camera_2d(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<Camera2DData>(entity)?;
    Some(json!({
        "zoom": data.zoom,
        "is_default_camera": data.is_default_camera
    }))
}

fn deserialize_camera_2d(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let zoom = data.get("zoom").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;

    let is_default_camera = data
        .get("is_default_camera")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    entity_commands.insert((
        Camera2d,
        Camera2DData {
            zoom,
            is_default_camera,
        },
    ));
}

fn inspect_camera_2d(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<Camera2DData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Zoom
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Zoom");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.zoom)
                            .speed(0.01)
                            .range(0.1..=10.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Is Default Camera
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Default Camera");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.is_default_camera, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Camera Rig
// ============================================================================

fn add_camera_rig(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        Camera3d::default(),
        CameraRigData::default(),
    ));
}

fn remove_camera_rig(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<Camera3d>()
        .remove::<Camera>()
        .remove::<CameraRigData>();
}

fn has_camera_rig(world: &World, entity: Entity) -> bool {
    world.get::<CameraRigData>(entity).is_some()
}

fn serialize_camera_rig(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<CameraRigData>(entity)?;
    Some(json!({
        "distance": data.distance,
        "height": data.height,
        "horizontal_offset": data.horizontal_offset,
        "fov": data.fov,
        "follow_smoothing": data.follow_smoothing,
        "look_smoothing": data.look_smoothing,
        "is_default_camera": data.is_default_camera
    }))
}

fn deserialize_camera_rig(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let rig_data = CameraRigData {
        distance: data
            .get("distance")
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0) as f32,
        height: data.get("height").and_then(|v| v.as_f64()).unwrap_or(2.0) as f32,
        horizontal_offset: data
            .get("horizontal_offset")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
        fov: data.get("fov").and_then(|v| v.as_f64()).unwrap_or(60.0) as f32,
        follow_smoothing: data
            .get("follow_smoothing")
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0) as f32,
        look_smoothing: data
            .get("look_smoothing")
            .and_then(|v| v.as_f64())
            .unwrap_or(10.0) as f32,
        is_default_camera: data
            .get("is_default_camera")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };

    entity_commands.insert((Camera3d::default(), rig_data));
}

fn inspect_camera_rig(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<CameraRigData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Distance
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Distance");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.distance)
                            .speed(0.1)
                            .range(0.5..=50.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Height
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Height");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.height)
                            .speed(0.1)
                            .range(-10.0..=20.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Horizontal Offset
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Horizontal Offset");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.horizontal_offset)
                            .speed(0.1)
                            .range(-10.0..=10.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // FOV
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("FOV");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.fov)
                            .speed(0.5)
                            .range(10.0..=170.0)
                            .suffix("\u{00b0}"),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Follow Smoothing
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Follow Smoothing");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.follow_smoothing)
                            .speed(0.1)
                            .range(0.0..=20.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Look Smoothing
    property_row(ui, 5, |ui| {
        ui.horizontal(|ui| {
            ui.label("Look Smoothing");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.look_smoothing)
                            .speed(0.1)
                            .range(0.0..=20.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Is Default Camera
    property_row(ui, 6, |ui| {
        ui.horizontal(|ui| {
            ui.label("Default Camera");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.is_default_camera, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}
