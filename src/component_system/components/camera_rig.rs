//! Camera rig component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::CameraRigData;
use crate::ui::property_row;

use egui_phosphor::regular::VIDEO_CAMERA;

// ============================================================================
// Custom Add/Remove/Deserialize
// ============================================================================

fn add_camera_rig(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        Camera3d::default(),
        Msaa::Off,
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

    entity_commands.insert((Camera3d::default(), Msaa::Off, rig_data));
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_camera_rig(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    // Render camera preview before borrowing component data
    super::camera_3d::render_camera_preview(ui, world);

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

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(CameraRigData {
        type_id: "camera_rig",
        display_name: "Camera Rig",
        category: ComponentCategory::Camera,
        icon: VIDEO_CAMERA,
        priority: 2,
        conflicts_with: ["camera_3d", "camera_2d"],
        custom_inspector: inspect_camera_rig,
        custom_add: add_camera_rig,
        custom_remove: remove_camera_rig,
        custom_deserialize: deserialize_camera_rig,
    }));
}
