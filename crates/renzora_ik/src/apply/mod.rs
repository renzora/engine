//! The systems that read a pose out of the hierarchy, hand it to a solver in
//! [`crate::solve`] and write the answer back. Ordered in [`crate::IkPlugin`]:
//! copy constraints, foot IK, two-bone IK, look-at, rotation limits, then
//! spring bones, so limits clamp what the IK asked for and the springs hang off
//! the final pose.

pub mod constraints;
pub mod foot;
pub mod spring;
#[cfg(test)]
mod tests;

use crate::names::NameLookup;
use crate::pose::Pose;
use bevy::prelude::*;

/// World position of a constraint's target: the named entity if it resolves,
/// else the fallback position.
pub(crate) fn target_point(
    name: &str,
    fallback: Vec3,
    near: Entity,
    lookup: &NameLookup,
    names: &Query<&Name>,
    pose: &Pose,
) -> (Vec3, Option<Transform>) {
    if let Some(world) = lookup.find(name, near, names, pose.parents).and_then(|e| pose.world(e)) {
        return (world.translation, Some(world));
    }
    (fallback, None)
}
