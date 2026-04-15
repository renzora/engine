//! Kinematic slide primitive.
//!
//! The built-in character controller was removed — script- and blueprint-driven
//! kinematic bodies use `shape_cast_slide` through the `kinematic_slide`
//! script action.

use avian3d::prelude::*;
use bevy::prelude::*;

/// Result of a [`shape_cast_slide`] call.
#[derive(Clone, Copy, Debug, Default)]
pub struct SlideResult {
    /// The movement that was actually applied after collide-and-slide.
    pub actual_delta: Vec3,
    /// True if a downward probe found ground at the end of the slide.
    pub grounded: bool,
    /// True if the slide hit a wall (non-walkable surface) during iteration.
    pub hit_wall: bool,
    /// Normal of the last ground hit (or `Vec3::Y` if airborne).
    pub ground_normal: Vec3,
}

/// Collide-and-slide: move a shape by `desired_delta`, clipping against hits
/// and retrying along the remaining tangent up to a fixed iteration budget.
/// Returns the effective delta and a grounded flag.
///
/// The caller is responsible for applying `result.actual_delta` to the
/// `Transform` (this function only does the query math).
pub fn shape_cast_slide(
    spatial_query: &SpatialQuery,
    shape: &Collider,
    origin: Vec3,
    rotation: Quat,
    desired_delta: Vec3,
    max_slope_deg: f32,
    filter: &SpatialQueryFilter,
) -> SlideResult {
    const MAX_ITERS: usize = 4;
    const SKIN: f32 = 0.002;

    let mut remaining = desired_delta;
    let mut origin = origin;
    let mut actual = Vec3::ZERO;
    let mut hit_wall = false;
    let max_slope_rad = max_slope_deg.to_radians();

    for _ in 0..MAX_ITERS {
        let dist = remaining.length();
        if dist < 1e-5 {
            break;
        }
        let dir = remaining / dist;
        let Ok(dir3) = Dir3::new(dir) else { break };

        let hit = spatial_query.cast_shape(
            shape,
            origin,
            rotation,
            dir3,
            &ShapeCastConfig {
                max_distance: dist + SKIN,
                ignore_origin_penetration: true,
                ..Default::default()
            },
            filter,
        );

        match hit {
            None => {
                actual += remaining;
                origin += remaining;
                break;
            }
            Some(h) => {
                let travel = (h.distance - SKIN).max(0.0);
                let moved = dir * travel;
                actual += moved;
                origin += moved;

                let angle = h.normal1.angle_between(Vec3::Y);
                if angle > max_slope_rad {
                    hit_wall = true;
                }

                let consumed = travel;
                let leftover_mag = (dist - consumed).max(0.0);
                let tangent = remaining - h.normal1 * remaining.dot(h.normal1);
                if tangent.length_squared() < 1e-8 {
                    break;
                }
                remaining = tangent.normalize() * leftover_mag;
            }
        }
    }

    // Downward probe to decide grounded at the final position.
    let mut grounded = false;
    let mut ground_normal = Vec3::Y;
    if let Ok(down) = Dir3::new(Vec3::NEG_Y) {
        if let Some(h) = spatial_query.cast_shape(
            shape,
            origin,
            rotation,
            down,
            &ShapeCastConfig {
                max_distance: 0.15,
                ignore_origin_penetration: true,
                ..Default::default()
            },
            filter,
        ) {
            let angle = h.normal1.angle_between(Vec3::Y);
            if angle <= max_slope_rad {
                grounded = true;
                ground_normal = h.normal1;
            }
        }
    }

    SlideResult {
        actual_delta: actual,
        grounded,
        hit_wall,
        ground_normal,
    }
}
