use bevy::prelude::*;

use crate::core::{EditorState, MainCamera, ViewportCamera};
use crate::gizmo::editor_camera_layers;
use crate::viewport::ViewportImage;

/// Marker for editor-only entities (not saved to scene)
#[derive(Component)]
pub struct EditorOnly;

/// Marker for the UI camera (used for egui rendering)
#[derive(Component)]
pub struct UiCamera;

/// Set up the editor camera
pub fn setup_editor_camera(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    viewport_image: &ViewportImage,
    editor_state: &EditorState,
) {
    // Camera that renders to the viewport texture
    // Position calculated from orbit parameters
    let cam_pos = editor_state.orbit_focus
        + Vec3::new(
            editor_state.orbit_distance * editor_state.orbit_pitch.cos() * editor_state.orbit_yaw.sin(),
            editor_state.orbit_distance * editor_state.orbit_pitch.sin(),
            editor_state.orbit_distance * editor_state.orbit_pitch.cos() * editor_state.orbit_yaw.cos(),
        );

    // Calculate initial aspect ratio from viewport size
    let aspect = if editor_state.viewport_size[1] > 0.0 {
        editor_state.viewport_size[0] / editor_state.viewport_size[1]
    } else {
        16.0 / 9.0 // Default aspect ratio
    };

    commands.spawn((
        Camera3d::default(),
        Camera {
            target: viewport_image.0.clone().into(),
            clear_color: ClearColorConfig::Custom(Color::srgb(0.15, 0.15, 0.18)),
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            aspect_ratio: aspect,
            ..default()
        }),
        Transform::from_translation(cam_pos).looking_at(editor_state.orbit_focus, Vec3::Y),
        MainCamera,
        ViewportCamera,
        EditorOnly,
        // Render both scene (layer 0) and gizmos (layer 1)
        editor_camera_layers(),
    ));

    // Add a directional light for the editor
    commands.spawn((
        DirectionalLight {
            illuminance: 15000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.4, 0.0)),
        EditorOnly,
    ));
}
