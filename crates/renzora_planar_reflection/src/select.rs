//! Which (surface, camera) pairs render this frame.
//!
//! Every pair that renders is a full extra scene render, so this is where the
//! cost is decided: a surface the camera cannot see, is behind, or is too far
//! from costs nothing, and of the rest only the nearest
//! [`crate::PlanarReflectionSettings::max_active`] render.
//!
//! Pure functions over plain data, so the tests can drive every branch.

use bevy::camera::primitives::{Aabb, Frustum};
use bevy::camera::visibility::RenderLayers;
use bevy::math::{Affine3A, Vec3};
use bevy::prelude::Entity;
use renzora::core::viewport_types::{
    VIEWPORT_2D_GRID_LAYER_BASE, VIEWPORT_3D_GIZMO_LAYER_BASE, VIEWPORT_COUNT,
};

use crate::math::ReflectionPlane;

/// How far in front of the plane the camera must be before the surface counts
/// as facing it. Closer than this the surface is edge-on: it covers no pixels
/// worth a render, and the mirror camera would sit almost on top of the real
/// one, which is also the distance at which the overlay's per-view gate could
/// no longer tell the two apart.
pub const FACING_EPSILON: f32 = 0.01;

/// Why a pair does not render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// The camera is behind the surface, or level with it.
    FacingAway,
    /// Beyond the surface's `max_distance`.
    TooFar,
    /// Outside the camera's frustum.
    OffScreen,
}

/// One reflecting surface as the selection pass sees it.
pub struct SurfaceView<'a> {
    pub plane: ReflectionPlane,
    pub world_from_local: &'a Affine3A,
    /// The mesh's local bounds. `None` until the mesh has loaded.
    pub aabb: Option<&'a Aabb>,
    pub max_distance: Option<f32>,
}

/// Decide whether `surface` is worth rendering for a camera at `camera_pos`
/// with `frustum`, and if so how far away it is (the sort key for the cap).
pub fn evaluate(surface: &SurfaceView, camera_pos: Vec3, frustum: &Frustum) -> Result<f32, Skip> {
    if surface.plane.signed_distance(camera_pos) <= FACING_EPSILON {
        return Err(Skip::FacingAway);
    }
    let Some(aabb) = surface.aabb else {
        // No bounds yet means no mesh yet, which means nothing on screen.
        return Err(Skip::OffScreen);
    };
    let distance = distance_to_obb(camera_pos, aabb, surface.world_from_local);
    if surface.max_distance.is_some_and(|max| distance > max) {
        return Err(Skip::TooFar);
    }
    if !frustum.intersects_obb(aabb, surface.world_from_local, true, true) {
        return Err(Skip::OffScreen);
    }
    Ok(distance)
}

/// Distance from `point` to the nearest point of an oriented box. Measured to
/// the box rather than the entity's origin so a large floor you are standing
/// on counts as near, not as ten metres away at its centre.
///
/// Clamped in local space and measured in world space, which stays right under
/// non-uniform scale.
pub fn distance_to_obb(point: Vec3, aabb: &Aabb, world_from_local: &Affine3A) -> f32 {
    let det = world_from_local.matrix3.determinant();
    if det.abs() < 1e-12 {
        return Vec3::from(world_from_local.translation).distance(point);
    }
    let local = world_from_local.inverse().transform_point3(point);
    let clamped = local.clamp(aabb.min().into(), aabb.max().into());
    world_from_local.transform_point3(clamped).distance(point)
}

/// A pair that passed [`evaluate`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub surface: Entity,
    pub camera: Entity,
    pub distance: f32,
}

/// The nearest `cap` candidates.
///
/// Ties break on entity ids so the chosen set cannot flicker between two
/// equidistant surfaces from one frame to the next as query order shuffles.
pub fn select_active(mut candidates: Vec<Candidate>, cap: usize) -> Vec<Candidate> {
    candidates.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then_with(|| a.surface.to_bits().cmp(&b.surface.to_bits()))
            .then_with(|| a.camera.to_bits().cmp(&b.camera.to_bits()))
    });
    candidates.truncate(cap);
    candidates
}

/// Whether a capture with `update_every` re-renders on `frame`. `stagger`
/// spreads surfaces sharing a rate across frames, so ten surfaces at "every
/// 2nd frame" cost five renders each frame rather than ten and then none.
pub fn update_due(frame: u64, update_every: u32, stagger: u64) -> bool {
    let every = u64::from(update_every.max(1));
    frame.wrapping_add(stagger).is_multiple_of(every)
}

/// Whether `layer` holds editor chrome rather than scene content: the shared
/// world-space overlays (1), the per-slot 2D grids and the per-slot 3D gizmos.
/// See the layer map in `renzora::core::viewport_types`.
pub fn is_editor_layer(layer: usize) -> bool {
    layer == 1
        || (VIEWPORT_2D_GRID_LAYER_BASE..VIEWPORT_2D_GRID_LAYER_BASE + VIEWPORT_COUNT)
            .contains(&layer)
        || (VIEWPORT_3D_GIZMO_LAYER_BASE..VIEWPORT_3D_GIZMO_LAYER_BASE + VIEWPORT_COUNT)
            .contains(&layer)
}

/// The layers a mirror camera renders for a viewing camera on `source`: the
/// same scene content, none of the editor chrome. A gizmo handle or a light
/// icon reflected in a mirror reads as an object in the level.
pub fn scene_layers(source: Option<&RenderLayers>) -> RenderLayers {
    let source = source.cloned().unwrap_or_default();
    let mut layers = RenderLayers::none();
    for layer in source.iter() {
        if !is_editor_layer(layer) {
            layers = layers.with(layer);
        }
    }
    layers
}
