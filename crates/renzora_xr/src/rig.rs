//! The visible player rig: procedural controller wands tracked to the grip
//! poses, a head-tracked desktop mirror camera, and suppression of the
//! scene's own flat cameras (in VR the head is the camera — an authored
//! `Camera3d` would otherwise fight the mirror for the desktop window and
//! render the whole scene an extra time).

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy_mod_xr::camera::XrCamera;
use bevy_xr_utils::tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip, XrTrackedView};

use crate::VrConfig;

/// Marker: the desktop mirror camera (head-tracked spectator view rendering
/// to the flat window).
#[derive(Component)]
pub struct VrMirrorCamera;

/// Marker: part of a controller wand visual.
#[derive(Component)]
pub struct VrControllerVisual;

pub(crate) fn register(app: &mut App) {
    app.add_systems(Startup, spawn_rig);
    app.add_systems(Update, (apply_visibility_config, suppress_flat_scene_cameras));
}

/// Spawn the controller wands and the mirror camera once at boot. The
/// tracking-utils systems position the grip/view marker entities every frame
/// while the session runs; before that they just sit at the origin (hidden
/// until first tracking data arrives is not worth the plumbing for a demo).
fn spawn_rig(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // ── Controller wands ────────────────────────────────────────────────
    // A stubby capsule "handle" plus an emissive tip sphere, tinted per hand
    // (left = blue, right = orange) so handedness reads instantly in-headset.
    let handle_mesh = meshes.add(Capsule3d::new(0.015, 0.09));
    let tip_mesh = meshes.add(Sphere::new(0.012));
    let handle_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.15, 0.17),
        perceptual_roughness: 0.7,
        ..Default::default()
    });

    let mut spawn_wand = |grip_side: &str, tint: Color| {
        let tip_material = materials.add(StandardMaterial {
            base_color: tint,
            emissive: tint.to_linear() * 2.0,
            ..Default::default()
        });
        let root = commands
            .spawn((
                Name::new(format!("VR Controller ({grip_side})")),
                Transform::default(),
                Visibility::default(),
                VrControllerVisual,
            ))
            .id();
        // Grip pose: +Z points back along the handle, so tilt the capsule to
        // sit in the fist like a torch and push the tip forward.
        let handle = commands
            .spawn((
                Mesh3d(handle_mesh.clone()),
                MeshMaterial3d(handle_material.clone()),
                Transform::from_rotation(Quat::from_rotation_x(1.1))
                    .with_translation(Vec3::new(0.0, -0.01, 0.02)),
                VrControllerVisual,
            ))
            .id();
        let tip = commands
            .spawn((
                Mesh3d(tip_mesh.clone()),
                MeshMaterial3d(tip_material),
                Transform::from_translation(Vec3::new(0.0, 0.0, -0.05)),
                VrControllerVisual,
            ))
            .id();
        commands.entity(root).add_children(&[handle, tip]);
        root
    };

    let left = spawn_wand("Left", Color::srgb(0.25, 0.55, 1.0));
    let right = spawn_wand("Right", Color::srgb(1.0, 0.55, 0.2));
    commands.entity(left).insert(XrTrackedLeftGrip);
    commands.entity(right).insert(XrTrackedRightGrip);

    // ── Desktop mirror camera ───────────────────────────────────────────
    // Head-tracked spectator view into the flat window. `XrTrackedView`
    // snaps it to the HMD pose each frame; rendering to the window is
    // independent of the eye cameras' swapchain targets. Ordered after the
    // eyes so frame timing stays headset-first.
    commands.spawn((
        Name::new("VR Desktop Mirror"),
        Camera3d::default(),
        Camera {
            order: 100,
            ..Default::default()
        },
        RenderTarget::Window(bevy::window::WindowRef::Primary),
        Transform::default(),
        XrTrackedView,
        VrMirrorCamera,
    ));
}

/// Apply the [`VrConfig`] visibility toggles (mirror on/off, wands on/off)
/// every frame — cheap, and keeps the knobs live-editable.
fn apply_visibility_config(
    config: Res<VrConfig>,
    mut mirror: Query<&mut Camera, With<VrMirrorCamera>>,
    mut wands: Query<&mut Visibility, With<VrControllerVisual>>,
) {
    if !config.is_changed() {
        return;
    }
    for mut camera in mirror.iter_mut() {
        camera.is_active = config.desktop_mirror;
    }
    for mut visibility in wands.iter_mut() {
        *visibility = if config.controller_visuals {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Deactivate authored flat 3D cameras that render to the window. Scenes
/// carry their own game camera for flat play; in VR it would render the scene
/// an extra time and contend with the mirror camera for the desktop window.
/// Render-to-texture cameras (script-driven monitors etc.) are left alone —
/// only window-targeting cameras conflict. Runs every frame so cameras
/// arriving with scene loads are caught too.
fn suppress_flat_scene_cameras(
    mut cameras: Query<
        (&mut Camera, Option<&RenderTarget>),
        (
            With<Camera3d>,
            Without<XrCamera>,
            Without<VrMirrorCamera>,
        ),
    >,
) {
    for (mut camera, target) in cameras.iter_mut() {
        // No RenderTarget component = the default (primary window) target.
        let windowed = target.is_none_or(|t| matches!(t, RenderTarget::Window(_)));
        if camera.is_active && windowed {
            camera.is_active = false;
        }
    }
}
