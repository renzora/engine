//! 2D camera system for the editor viewport
//!
//! Provides an orthographic camera for editing 2D scenes and UI nodes.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::input::mouse::{MouseMotion, MouseWheel};

use crate::core::{ViewportMode, ViewportState};
use crate::gizmo::editor_camera_layers;
use crate::scene::EditorOnly;
use super::{Camera2DState, ViewportImage};

/// Marker component for the 2D editor camera
#[derive(Component)]
pub struct Editor2DCamera;

/// Set up the 2D editor camera
pub fn setup_editor_camera_2d(
    commands: &mut Commands,
    viewport_image: &ViewportImage,
    _viewport: &ViewportState,
    camera2d_state: &Camera2DState,
) {
    // Calculate initial camera transform from 2D state
    let cam_pos = Vec3::new(
        camera2d_state.pan_offset.x,
        camera2d_state.pan_offset.y,
        1000.0, // Far enough to see everything
    );

    commands.spawn((
        Camera2d,
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.12, 0.12, 0.14)),
            order: -1, // Render before 3D camera
            is_active: false, // Start inactive, activated when switching to 2D mode
            ..default()
        },
        RenderTarget::Image(viewport_image.0.clone().into()),
        Transform::from_translation(cam_pos),
        Editor2DCamera,
        EditorOnly,
        editor_camera_layers(),
        Name::new("Editor 2D Camera"),
    ));
}

/// Camera controller for 2D mode
/// Handles panning with middle mouse button and zooming with scroll wheel
pub fn camera2d_controller(
    viewport: Res<ViewportState>,
    mut camera2d_state: ResMut<Camera2DState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<Editor2DCamera>>,
) {
    // Only active in 2D mode
    if viewport.viewport_mode != ViewportMode::Mode2D {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    let Ok((mut transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    if !viewport.hovered {
        mouse_motion.clear();
        scroll_events.clear();
        return;
    }

    let middle_pressed = mouse_button.pressed(MouseButton::Middle);
    let middle_just_pressed = mouse_button.just_pressed(MouseButton::Middle);

    // Clear accumulated events on first frame of button press to prevent jump
    if middle_just_pressed {
        mouse_motion.clear();
        camera2d_state.is_panning = true;
        return;
    }

    if mouse_button.just_released(MouseButton::Middle) {
        camera2d_state.is_panning = false;
    }

    // Handle zoom with scroll wheel
    for ev in scroll_events.read() {
        let zoom_factor = if ev.y > 0.0 { 1.1 } else { 0.9 };
        camera2d_state.zoom = (camera2d_state.zoom * zoom_factor).clamp(0.1, 10.0);
    }

    // Handle pan with middle mouse drag
    if middle_pressed {
        for ev in mouse_motion.read() {
            // Pan speed is inversely proportional to zoom
            let pan_speed = 1.0 / camera2d_state.zoom;
            camera2d_state.pan_offset.x -= ev.delta.x * pan_speed;
            camera2d_state.pan_offset.y += ev.delta.y * pan_speed;
        }
    } else {
        mouse_motion.clear();
    }

    // Apply state to camera transform
    transform.translation.x = camera2d_state.pan_offset.x;
    transform.translation.y = camera2d_state.pan_offset.y;

    // Update orthographic projection scale based on zoom
    if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scale = 1.0 / camera2d_state.zoom;
    }
}

/// System to toggle camera activation based on viewport mode
pub fn toggle_viewport_cameras(
    viewport: Res<ViewportState>,
    mut camera_3d_query: Query<&mut Camera, (With<crate::core::ViewportCamera>, Without<Editor2DCamera>)>,
    mut camera_2d_query: Query<&mut Camera, (With<Editor2DCamera>, Without<crate::core::ViewportCamera>)>,
) {
    if !viewport.is_changed() {
        return;
    }

    let is_2d_mode = viewport.viewport_mode == ViewportMode::Mode2D;

    // Toggle 3D camera
    for mut camera in camera_3d_query.iter_mut() {
        camera.is_active = !is_2d_mode;
    }

    // Toggle 2D camera
    for mut camera in camera_2d_query.iter_mut() {
        camera.is_active = is_2d_mode;
    }
}
