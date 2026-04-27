//! Runtime camera spawning and render target syncing.

use bevy::prelude::*;
use bevy::camera::{Camera, RenderTarget};
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::prepass::NormalPrepass;
use bevy::light::AtmosphereEnvironmentMapLight;
use bevy::pbr::{Atmosphere, AtmosphereSettings, ScatteringMedium};
use bevy::render::view::Hdr;
use renzora::viewport_types::EditorCameraMatrix;
use crate::{EditorCamera, EditorLocked, HideInHierarchy, ViewportRenderTarget};

/// Spawns the editor's 3D scene-navigation camera.
///
/// If `ViewportRenderTarget` already has an image (editor mode),
/// the camera renders to it. Otherwise it renders to the window.
/// The camera is hidden from the hierarchy and locked from editing.
///
/// Render-effect components (`Atmosphere`, `AtmosphereSettings`,
/// `AtmosphereEnvironmentMapLight`, `Msaa::Off`, etc.) are attached at
/// spawn so Bevy 0.18's atmosphere/sky pipeline can lock in its bind
/// group layout once and never need to grow it. Trying to add atmosphere
/// at runtime crashes wgpu with "20 vs 23 bindings" — Bevy specializes
/// the layout per-camera at first render, and atmosphere bindings are
/// gated on whether the component existed at that moment.
///
/// `EffectRouting` + `renzora_atmosphere::sync_atmosphere` then *update*
/// these components in-place from a `WorldEnvironment` source entity (or
/// any entity with `AtmosphereComponentSettings`), giving us one logical
/// source of truth that drives both editor and play cameras identically.
/// The plugin replaces, never removes — see its file for the why.
pub fn spawn_editor_camera(
    mut commands: Commands,
    render_target: Res<ViewportRenderTarget>,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
) {
    let default_medium = mediums.add(ScatteringMedium::default());

    let mut entity = commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            far: 100_000.0,
            ..default()
        }),
        Transform::from_xyz(5.0, 4.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        EditorCamera,
        HideInHierarchy,
        EditorLocked,
        RenderLayers::from_layers(&[0, 1]),
        Name::new("Editor Camera"),
        Hdr,
        Atmosphere {
            bottom_radius: 6_360_000.0,
            top_radius: 6_460_000.0,
            ground_albedo: Vec3::splat(0.3),
            medium: default_medium,
        },
        AtmosphereSettings::default(),
        // IBL is off by default — `intensity: 0.0` keeps the bind-group
        // slots present (Bevy 0.18 won't let us add this component at
        // runtime without a pipeline crash) but contributes nothing
        // visually. Adding an `EnvironmentMapComponentSettings` to any
        // entity in the scene routes a non-zero intensity onto the
        // camera; removing it pushes intensity back to 0. See
        // `renzora_environment_map`.
        AtmosphereEnvironmentMapLight {
            intensity: 0.0,
            ..default()
        },
        // Atmosphere/sky binds depth as non-multisampled (binding 13);
        // any MSAA on the same camera trips a wgpu validation crash.
        Msaa::Off,
        // Force the prepass to carry world normals. Without this,
        // `pbr_fragment.wgsl::pbr_input_from_vertex_output` fails to compile
        // for any material with `alpha_mode = Mask` because the prepass
        // calls into it to do alpha cutout, but the prepass `VertexOutput`
        // gates `world_normal` behind `NORMAL_PREPASS_OR_DEFERRED_PREPASS`.
        NormalPrepass,
    ));

    if let Some(ref image) = render_target.image {
        info!("[camera] Editor camera spawned with offscreen render target");
        entity.insert(RenderTarget::Image(image.clone().into()));
    } else {
        info!("[camera] Editor camera spawned rendering to window (no viewport image yet)");
    }
}

/// Watches for changes to `ViewportRenderTarget` and updates the camera accordingly.
///
/// Only acts when an image handle is set (editor mode). When `None`, the camera
/// keeps its default window target — we never remove `RenderTarget`.
pub fn sync_camera_render_target(
    render_target: Res<ViewportRenderTarget>,
    cameras: Query<Entity, With<EditorCamera>>,
    mut commands: Commands,
) {
    if !render_target.is_changed() {
        return;
    }

    if let Some(ref image) = render_target.image {
        info!("[camera] ViewportRenderTarget changed — redirecting editor camera to offscreen image");
        for entity in &cameras {
            commands
                .entity(entity)
                .insert(RenderTarget::Image(image.clone().into()));
        }
    } else {
        info!("[camera] ViewportRenderTarget changed — but image is None");
    }
}

/// Cache the editor camera's clip-from-world matrix into a resource each frame,
/// so overlay systems (grid, gizmos) can CPU-project geometry without querying
/// the camera themselves (which requires mutable World access).
pub fn update_editor_camera_matrix(
    cameras: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut mat: ResMut<EditorCameraMatrix>,
) {
    let Ok((camera, transform)) = cameras.single() else {
        mat.valid = false;
        return;
    };
    let view_from_world = transform.affine().inverse();
    let clip_from_view = camera.clip_from_view();
    mat.clip_from_world = clip_from_view * Mat4::from(view_from_world);
    mat.world_from_clip = mat.clip_from_world.inverse();
    mat.cam_pos = transform.translation();
    mat.cam_forward = *transform.forward();
    mat.valid = true;
}
