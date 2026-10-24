//! Foot planting math.
//!
//! A walk cycle is authored on flat ground at the character's own height. On
//! uneven ground the only thing that changes is how far the floor under each
//! foot sits above or below that plane, so the whole problem reduces to one
//! number per foot: its *ground offset*. Adding it to the animated ankle keeps
//! everything the clip did (a lifted swing foot stays lifted, by the same
//! amount, over the step it is crossing), with no separate "foot height" to
//! tune and no snapping a raised foot down onto the floor.
//!
//! The pelvis then drops by the most negative offset, so the leg over the
//! lower ground can still reach it. It never rises: a foot on higher ground is
//! met by bending that leg, which is what a person does on a stair.

use bevy::math::{Quat, Vec3};

/// How far the ground under a foot sits above (`+`) or below (`-`) the
/// character's own floor plane.
pub fn ground_offset(ground_height: f32, floor_height: f32) -> f32 {
    ground_height - floor_height
}

/// How far to move the pelvis along up (always `<= 0`): down by the lowest
/// foot's offset, but no further than `max_drop`, past which the character
/// would be crouching on a ledge it should be stepping off.
pub fn pelvis_drop(offsets: &[f32], max_drop: f32) -> f32 {
    let lowest = offsets.iter().copied().fold(0.0_f32, f32::min);
    lowest.max(-max_drop.abs())
}

/// Where the ankle goes: the animated ankle carried along `up` by the foot's
/// ground offset.
pub fn plant(animated_ankle: Vec3, up: Vec3, offset: f32) -> Vec3 {
    animated_ankle + up * offset
}

/// The foot's world rotation tilted from `up` onto the ground `normal`, by at
/// most `max_slope` radians, so a foot on a steep rock does not stand on its
/// toes at ninety degrees.
pub fn align(rotation: Quat, up: Vec3, normal: Vec3, max_slope: f32) -> Quat {
    let Some(n) = normal.try_normalize() else {
        return rotation;
    };
    let n = super::aim::clamp_direction(up, n, max_slope);
    (Quat::from_rotation_arc(up, n) * rotation).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_6;

    #[test]
    fn flat_ground_changes_nothing() {
        let offsets = [ground_offset(0.0, 0.0), ground_offset(0.0, 0.0)];
        assert_eq!(pelvis_drop(&offsets, 0.5), 0.0);
        let ankle = Vec3::new(0.1, 0.08, 0.2);
        assert_eq!(plant(ankle, Vec3::Y, offsets[0]), ankle);
    }

    #[test]
    fn the_pelvis_drops_for_the_lower_foot() {
        let offsets = [ground_offset(-0.2, 0.0), ground_offset(0.1, 0.0)];
        assert!((pelvis_drop(&offsets, 0.5) + 0.2).abs() < 1e-6);
    }

    #[test]
    fn the_pelvis_never_rises() {
        let offsets = [0.15, 0.3];
        assert_eq!(pelvis_drop(&offsets, 0.5), 0.0);
    }

    #[test]
    fn the_drop_is_capped() {
        assert!((pelvis_drop(&[-2.0, 0.0], 0.4) + 0.4).abs() < 1e-6);
    }

    #[test]
    fn a_lifted_foot_keeps_its_lift_over_a_step() {
        // Mid-swing, 0.25 above the floor, crossing a 0.1 step.
        let ankle = Vec3::new(0.0, 0.25, 0.3);
        let planted = plant(ankle, Vec3::Y, ground_offset(1.1, 1.0));
        assert!((planted.y - 0.35).abs() < 1e-6);
    }

    #[test]
    fn feet_tilt_onto_slopes_up_to_the_limit() {
        let slope = Vec3::new(0.0, 1.0, 1.0).normalize(); // 45 degrees
        let r = align(Quat::IDENTITY, Vec3::Y, slope, 1.0);
        assert!((r * Vec3::Y).distance(slope) < 1e-5);
        let r = align(Quat::IDENTITY, Vec3::Y, slope, FRAC_PI_6);
        assert!(((r * Vec3::Y).dot(Vec3::Y).acos() - FRAC_PI_6).abs() < 1e-4);
    }
}
