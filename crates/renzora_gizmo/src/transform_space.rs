//! Parent-space-aware transform math shared by the tool gizmo and the modal
//! (G/R/S) transforms.
//!
//! Every gizmo gesture is expressed in **world space** (a world-space drag
//! delta, a world-space rotation axis, a world-space pivot), but the value we
//! actually write is the entity's **local** `Transform`. When an entity is
//! nested under a parent with its own rotation/scale — e.g. an imported GLB
//! whose meshes sit under a converted, `X = -90°` root — adding a world delta
//! straight onto the local translation moves it along the wrong axis, and
//! scaling/rotating about the local origin makes baked-vertex meshes fly off
//! instead of staying put. These helpers convert world-space intent into the
//! correct local-space write, and keep a chosen world pivot fixed.

use bevy::math::Affine3A;
use bevy::prelude::*;

/// The parent's world affine, derived from an entity's current world and local
/// transforms (`world = parent * local`). The parent doesn't move during a
/// gizmo gesture (only the selected entity does), so recomputing this from the
/// live transforms each frame still yields the true, stable parent frame.
pub(crate) fn parent_affine(entity_world: &GlobalTransform, local: &Transform) -> Affine3A {
    entity_world.affine() * local.compute_affine().inverse()
}

/// Translate by a world-space `world_delta`, written into the parent's local
/// frame. Returns the new local translation (caller may snap it). A no-op
/// conversion when unparented (parent is identity).
pub(crate) fn world_translation(start_t: Vec3, world_delta: Vec3, parent: &Affine3A) -> Vec3 {
    start_t + parent.inverse().transform_vector3(world_delta)
}

/// Rotate by `world_rot` (a world-space rotation) about `pivot_world`, writing
/// the result into the entity's local `Transform`. Translation is shifted so the
/// pivot stays fixed; scale is untouched.
pub(crate) fn pivot_rotation(
    t: &mut Transform,
    start_t: Vec3,
    start_r: Quat,
    start_s: Vec3,
    world_rot: Quat,
    pivot_world: Vec3,
    parent: &Affine3A,
) {
    let parent_inv = parent.inverse();
    let (_, parent_rot, _) = parent.to_scale_rotation_translation();
    // new world rotation = world_rot * (parent_rot * start_r); map back to local.
    t.rotation = parent_rot.inverse() * world_rot * parent_rot * start_r;
    // Rotate the entity's world origin about the pivot, then pull back to local.
    let world_origin = parent.transform_point3(start_t);
    let new_world_origin = pivot_world + world_rot * (world_origin - pivot_world);
    t.translation = parent_inv.transform_point3(new_world_origin);
    t.scale = start_s;
}

/// Set the entity's local `scale` to `new_scale` (scaling is always along the
/// entity's own axes), shifting translation so `pivot_world` stays fixed.
pub(crate) fn pivot_scale(
    t: &mut Transform,
    start_t: Vec3,
    start_r: Quat,
    start_s: Vec3,
    new_scale: Vec3,
    pivot_world: Vec3,
    parent: &Affine3A,
) {
    // Pivot expressed in the entity's pre-scale local frame: `s_v == start_s ⊙ v`
    // for the mesh point `v` that currently sits at the pivot.
    let q = parent.inverse().transform_point3(pivot_world);
    let s_v = start_r.inverse() * (q - start_t);
    // Per-axis factor (new / old), guarding a zero start scale.
    let f = Vec3::new(
        ratio(new_scale.x, start_s.x),
        ratio(new_scale.y, start_s.y),
        ratio(new_scale.z, start_s.z),
    );
    // Keeping the pivot point fixed requires T' = T - R·((f-1) ⊙ s_v).
    t.translation = start_t - (start_r * ((f - Vec3::ONE) * s_v));
    t.rotation = start_r;
    t.scale = new_scale;
}

fn ratio(new: f32, old: f32) -> f32 {
    if old.abs() > 1e-6 {
        new / old
    } else {
        1.0
    }
}
