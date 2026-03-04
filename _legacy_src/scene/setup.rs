#![allow(dead_code)]

use bevy::prelude::*;
use bevy::camera::RenderTarget;

use crate::console_info;
use crate::core::{MainCamera, ViewportCamera, OrbitCameraState, ViewportState};
use crate::gizmo::{editor_camera_layers, gizmo_overlay_layers, GizmoOverlayCamera};
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
    orbit: &OrbitCameraState,
    viewport: &ViewportState,
) {
    console_info!("Camera", "=== SETUP EDITOR CAMERA ===");

    // Camera that renders to the viewport texture
    // Position calculated from orbit parameters
    let cam_pos = orbit.focus
        + Vec3::new(
            orbit.distance * orbit.pitch.cos() * orbit.yaw.sin(),
            orbit.distance * orbit.pitch.sin(),
            orbit.distance * orbit.pitch.cos() * orbit.yaw.cos(),
        );

    console_info!("Camera", "Position: ({:.2}, {:.2}, {:.2})", cam_pos.x, cam_pos.y, cam_pos.z);
    console_info!("Camera", "Focus: ({:.2}, {:.2}, {:.2})", orbit.focus.x, orbit.focus.y, orbit.focus.z);
    console_info!("Camera", "Distance: {:.2}, Pitch: {:.2}, Yaw: {:.2}", orbit.distance, orbit.pitch, orbit.yaw);

    // Calculate initial aspect ratio from viewport size
    let aspect = if viewport.size[1] > 0.0 {
        viewport.size[0] / viewport.size[1]
    } else {
        16.0 / 9.0 // Default aspect ratio
    };

    console_info!("Camera", "Viewport size: {}x{}, aspect: {:.3}", viewport.size[0], viewport.size[1], aspect);
    console_info!("Camera", "Render target: viewport texture");
    console_info!("Camera", "MSAA: Off (required for MeshletPlugin)");
    console_info!("Camera", "Clear color: sRGB(0.15, 0.15, 0.18)");

    // Scene camera — renders layer 0 (scene objects, grid) with Solari when active
    let camera_entity = commands.spawn((
        Camera3d::default(),
        Msaa::Off, // MeshletPlugin requires explicit Msaa::Off
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.15, 0.15, 0.18)),
            ..default()
        },
        RenderTarget::Image(viewport_image.0.clone().into()),
        Projection::Perspective(PerspectiveProjection {
            aspect_ratio: aspect,
            ..default()
        }),
        Transform::from_translation(cam_pos).looking_at(orbit.focus, Vec3::Y),
        MainCamera,
        ViewportCamera,
        EditorOnly,
        // Scene only (layer 0) — gizmos are on a separate overlay camera
        editor_camera_layers(),
        Name::new("Main Viewport Camera"),
    )).id();

    // When compiled with solari, always use HDR + STORAGE_BINDING on the camera.
    // This matches the HDR viewport texture format and ensures the Solari-modified
    // render pipeline works from startup without needing a runtime format switch.
    #[cfg(feature = "solari")]
    {
        use bevy::render::render_resource::TextureUsages;
        use bevy::render::view::Hdr;
        use bevy::camera::CameraMainTextureUsages;

        commands.entity(camera_entity).insert((
            Hdr,
            CameraMainTextureUsages(
                TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::STORAGE_BINDING
            ),
        ));
        console_info!("Camera", "HDR + CameraMainTextureUsages(STORAGE_BINDING) added (solari feature compiled)");
    }

    // Gizmo overlay camera — renders layer 1 (gizmos) on top of scene, no Solari lighting
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderTarget::Image(viewport_image.0.clone().into()),
        Projection::Perspective(PerspectiveProjection {
            aspect_ratio: aspect,
            ..default()
        }),
        Transform::from_translation(cam_pos).looking_at(orbit.focus, Vec3::Y),
        GizmoOverlayCamera,
        EditorOnly,
        gizmo_overlay_layers(),
        Name::new("Gizmo Overlay Camera"),
    ));
    console_info!("Camera", "Gizmo overlay camera spawned (layer 1, order 1, no Solari)");

    console_info!("Camera", "=== CAMERA SETUP COMPLETE ===");
}
