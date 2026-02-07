//! Camera 3D component definition

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText, Vec2};

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::InspectorPanelRenderState;
use crate::register_component;
use crate::shared::CameraNodeData;
use crate::ui::{property_row, get_inspector_theme};

use egui_phosphor::regular::VIDEO_CAMERA;

// ============================================================================
// Custom Add/Remove/Deserialize
// ============================================================================

fn add_camera_3d(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        Camera3d::default(),
        Msaa::Off,
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
        Msaa::Off,
        CameraNodeData {
            fov,
            is_default_camera,
        },
    ));
}

// ============================================================================
// Camera Preview Helper (shared with camera_rig)
// ============================================================================

/// Render a camera preview image if available
pub(super) fn render_camera_preview(ui: &mut egui::Ui, world: &World) {
    let preview_texture_id = world
        .get_resource::<InspectorPanelRenderState>()
        .and_then(|s| s.camera_preview_texture_id);

    let available_width = ui.available_width();
    let preview_height = available_width * (9.0 / 16.0);

    if let Some(texture_id) = preview_texture_id {
        let image = egui::Image::new(egui::load::SizedTexture::new(
            texture_id,
            [available_width, preview_height],
        ));
        ui.add(image);
    } else {
        let theme_colors = get_inspector_theme(ui.ctx());
        egui::Frame::new()
            .fill(theme_colors.surface_faint)
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(available_width, preview_height));
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Preview loading...")
                            .color(theme_colors.text_disabled),
                    );
                });
            });
    }

    ui.add_space(4.0);
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_camera_3d(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    // Render camera preview before borrowing component data
    render_camera_preview(ui, world);

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
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(CameraNodeData {
        type_id: "camera_3d",
        display_name: "Camera 3D",
        category: ComponentCategory::Camera,
        icon: VIDEO_CAMERA,
        priority: 0,
        conflicts_with: ["camera_2d", "camera_rig"],
        custom_inspector: inspect_camera_3d,
        custom_add: add_camera_3d,
        custom_remove: remove_camera_3d,
        custom_deserialize: deserialize_camera_3d,
    }));
}
