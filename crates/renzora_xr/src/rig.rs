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

// (Editor viewport suspension lives editor-side: renzora_viewport's
// `sync_viewport_camera_activation` owns per-frame camera activity and now
// quiets everything but the parked atmosphere/IBL probe while
// `VrPlayState.active` — an XR-side override here would just fight it.)

/// The XR session is currently rendering to the headset.
fn session_active(play: Option<Res<renzora::VrPlayState>>) -> bool {
    play.is_some_and(|p| p.active)
}

/// Spawn the controller wands and the mirror camera once at boot. The
/// tracking-utils systems position the grip/view marker entities every frame
/// while the session runs; before that they just sit at the origin (hidden
/// until first tracking data arrives is not worth the plumbing for a demo).
fn spawn_rig(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    boot: Res<crate::XrBootMode>,
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
                // Editor-internal rig, not scene content: keep it out of the
                // hierarchy panel and the scene saver (descendant meshes are
                // hidden with it via the hierarchy's ancestor check).
                renzora::HideInHierarchy,
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
    //
    // Game mode (`--vr`) only: in an XR-capable EDITOR session the primary
    // window is the editor UI — a window-targeting 3D camera would draw the
    // scene over the chrome. The editor's own viewport panel is the mirror
    // there (it keeps rendering through the normal play-mode path).
    if !boot.game {
        return;
    }
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
        // Editor-internal spectator camera, not scene content.
        renzora::HideInHierarchy,
    ));
}

/// Apply the [`VrConfig`] visibility toggles (mirror on/off, wands on/off)
/// every frame — cheap, and keeps the knobs live-editable.
fn apply_visibility_config(
    config: Res<VrConfig>,
    play: Option<Res<renzora::VrPlayState>>,
    mut mirror: Query<&mut Camera, With<VrMirrorCamera>>,
    mut wands: Query<&mut Visibility, With<VrControllerVisual>>,
) {
    // Everything rig-side exists only while the headset session runs — wands
    // parked at the origin during flat editing would litter the scene.
    let active = play.is_some_and(|p| p.active);
    for mut camera in mirror.iter_mut() {
        let want = active && config.desktop_mirror;
        if camera.is_active != want {
            camera.is_active = want;
        }
    }
    let want = if active && config.controller_visuals {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in wands.iter_mut() {
        if *visibility != want {
            *visibility = want;
        }
    }
}

/// Deactivate authored flat 3D cameras that render to the window. Scenes
/// carry their own game camera for flat play; in VR it would render the scene
/// an extra time and contend with the mirror camera for the desktop window.
/// Render-to-texture cameras (script-driven monitors etc.) are left alone —
/// only window-targeting cameras conflict. Runs every frame so cameras
/// arriving with scene loads are caught too.
fn suppress_flat_scene_cameras(
    boot: Res<crate::XrBootMode>,
    play: Option<Res<renzora::VrPlayState>>,
    mut cameras: Query<
        (&mut Camera, Option<&RenderTarget>),
        (
            With<Camera3d>,
            Without<XrCamera>,
            Without<VrMirrorCamera>,
        ),
    >,
) {
    // Game mode: always (the headset is the only real view). Editor: only
    // while the session runs — flat editing must keep its cameras.
    if !boot.game && !session_active(play) {
        return;
    }
    for (mut camera, target) in cameras.iter_mut() {
        // No RenderTarget component = the default (primary window) target.
        let windowed = target.is_none_or(|t| matches!(t, RenderTarget::Window(_)));
        if camera.is_active && windowed {
            camera.is_active = false;
        }
    }
}
