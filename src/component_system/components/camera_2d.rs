//! Camera 2D component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::Camera2DData;
use crate::ui::property_row;

use egui_phosphor::regular::APERTURE;

// ============================================================================
// Custom Add/Remove/Deserialize
// ============================================================================

fn add_camera_2d(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        Camera2d,
        Msaa::Off,
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
        Msaa::Off,
        Camera2DData {
            zoom,
            is_default_camera,
        },
    ));
}

// ============================================================================
// Custom Inspector
// ============================================================================

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
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(Camera2DData {
        type_id: "camera_2d",
        display_name: "Camera 2D",
        category: ComponentCategory::Camera,
        icon: APERTURE,
        priority: 1,
        conflicts_with: ["camera_3d", "camera_rig"],
        custom_inspector: inspect_camera_2d,
        custom_add: add_camera_2d,
        custom_remove: remove_camera_2d,
        custom_deserialize: deserialize_camera_2d,
    }));
}
