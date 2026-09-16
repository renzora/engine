//! GPU occlusion culling — attaching Bevy's [`OcclusionCulling`] to the cameras
//! it is both wanted on and safe on.
//!
//! Frustum culling (Bevy's default, and on for everything here) drops what is
//! outside the camera's view. This drops what is inside the view but *behind*
//! something opaque: a room full of furniture costs nothing while the door is
//! shut. It is not the same as the depth test, which also rejects hidden
//! geometry but only after the GPU has transformed and skinned the vertices that
//! produced those fragments; occlusion culling tests bounding boxes against a
//! depth pyramid instead, so the work is never scheduled.
//!
//! The policy lives in [`OcclusionCullingEnabled`] (project config for a shipped
//! game, Settings → Viewport → Performance in the editor). This module is the
//! mechanism, and its whole job is knowing where *not* to apply it.
//!
//! # The three declines
//!
//! 1. **Deferred shading.** Upstream states `DeferredPrepass` together with
//!    `OcclusionCulling` is unspecified behaviour, so a deferred camera is
//!    stripped rather than merely skipped: the project's rendering mode can flip
//!    to Deferred at any point after a camera was set up, and `DeferredPrepass`
//!    also arrives later for other reasons (SSR).
//! 2. **Offscreen utility cameras.** Material and model thumbnails, studio
//!    previews, env bakes and the game-UI canvas all carry `IsolatedCamera`.
//!    Each one that opted in would allocate its own depth pyramid to cull a
//!    scene of one object, which is pure overhead. The same exclusion
//!    `ensure_contact_shadows_on_forward_cameras` uses.
//! 3. **No depth prepass.** Upstream ignores `OcclusionCulling` without one, so
//!    attaching it there would be a lie in the inspector rather than a crash.
//!    Every camera `renzora_engine::camera` spawns has the full prepass bundle,
//!    but a camera from a glTF node or a plugin need not.
//!
//! A fourth case needs nothing from us: on a platform with no GPU preprocessing
//! (WebGL2 has no compute shaders) Bevy marks the view `NoIndirectDrawing`, and
//! every occlusion-culling system filters those out, so the component sits inert
//! instead of misbehaving.
//!
//! # Why toggling is safe here, when so much else on a camera is spawn-locked
//!
//! Most of what this crate attaches to a camera has to be there from the first
//! render, because Bevy specializes the prepass pipeline once and a later
//! addition trips a wgpu validation crash (see `camera.rs`, and the contact
//! shadows note in `lib.rs`). Occlusion culling is not in that class: it changes
//! which render *phases* a view has and whether it owns a depth pyramid, both
//! rebuilt from the component every frame. Upstream's
//! `prepare_view_depth_pyramids` explicitly removes the pyramid from views that
//! stopped wanting it, which is only there to support exactly this.

use bevy::core_pipeline::prepass::{DeferredPrepass, DepthPrepass};
use bevy::prelude::*;
use bevy::render::occlusion_culling::OcclusionCulling;

use renzora::{CurrentProject, IsolatedCamera, OcclusionCullingEnabled};

/// Seed [`OcclusionCullingEnabled`] from the loaded project's `[rendering]`
/// block, for a shipped game. The editor mirrors its own viewport setting onto
/// the same resource instead (`renzora_level_presets::graphics_quality`).
///
/// Same shape as [`crate::graphics_quality::sync_runtime_graphics_quality`]:
/// idempotent, assigns only on a difference, and sits in `Update` so it picks
/// the value up whenever the project finishes loading.
pub fn sync_runtime_occlusion_culling(
    project: Option<Res<CurrentProject>>,
    mut enabled: ResMut<OcclusionCullingEnabled>,
) {
    let Some(project) = project else {
        return;
    };
    let want = project.config.rendering.occlusion_culling;
    if enabled.0 != want {
        enabled.0 = want;
        info!("[runtime] occlusion culling: {}", want);
    }
}

/// Add or remove [`OcclusionCulling`] on every 3D camera it belongs on.
///
/// Runs in `PostUpdate` beside the other camera safety nets, and deliberately
/// scans every frame with no `Added<Camera3d>` filter, for the reason
/// `ensure_deferred_prepass_on_cameras` documents: the inputs (the project's
/// rendering mode, the resource, whether a camera has picked up a
/// `DeferredPrepass` since) all change after a camera was spawned, and an
/// `Added` query would have looked exactly once and never revisited. The
/// component filters keep the scan cheap — a camera that already agrees with the
/// policy matches neither query.
pub fn ensure_occlusion_culling_on_cameras(
    enabled: Res<OcclusionCullingEnabled>,
    add_cameras: Query<
        Entity,
        (
            With<Camera3d>,
            With<DepthPrepass>,
            Without<OcclusionCulling>,
            Without<DeferredPrepass>,
            Without<IsolatedCamera>,
        ),
    >,
    // Cameras that must not keep it: the policy turned off, or the camera
    // acquired one of the disqualifying markers after it was attached.
    strip_cameras: Query<
        Entity,
        (
            With<OcclusionCulling>,
            Or<(
                With<DeferredPrepass>,
                With<IsolatedCamera>,
                Without<DepthPrepass>,
            )>,
        ),
    >,
    all_culling: Query<Entity, With<OcclusionCulling>>,
    mut commands: Commands,
) {
    if !enabled.0 {
        for entity in &all_culling {
            commands.entity(entity).remove::<OcclusionCulling>();
        }
        return;
    }

    for entity in &strip_cameras {
        commands.entity(entity).remove::<OcclusionCulling>();
    }
    for entity in &add_cameras {
        commands.entity(entity).try_insert(OcclusionCulling);
    }
}
