//! How a skeleton is drawn: one palette for every place that draws one (the
//! scene viewport's skeleton gizmo, the Animation workspace's stage), so a bone
//! looks the same wherever you meet it.
//!
//! Left is orange, right is teal and the centre line is yellow: the two sides
//! differ in lightness as well as hue, so they still read apart for someone who
//! cannot tell orange from teal. The selected bone takes the editor's accent
//! blue, and the one under the cursor a pale gold.

use bevy::prelude::Color;

/// Which side of the body a bone is on, from its name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoneSide {
    Left,
    Right,
    Centre,
}

/// The side a bone's name says it is on: a `left`/`right` anywhere in it
/// (`mixamorig:LeftForeArm`, `mixamorig_leftforearm`), or a `.L`/`_L`/`.R`/`_R`
/// suffix (`arm.L`, `face_lid_upper_r`).
pub fn bone_side(name: &str) -> BoneSide {
    let lower = name.to_ascii_lowercase();
    if lower.contains("left") {
        return BoneSide::Left;
    }
    if lower.contains("right") {
        return BoneSide::Right;
    }
    if lower.ends_with(".l") || lower.ends_with("_l") {
        return BoneSide::Left;
    }
    if lower.ends_with(".r") || lower.ends_with("_r") {
        return BoneSide::Right;
    }
    BoneSide::Centre
}

pub const BONE_LEFT: Color = Color::srgb(0.94, 0.64, 0.37);
pub const BONE_RIGHT: Color = Color::srgb(0.43, 0.78, 0.85);
pub const BONE_CENTRE: Color = Color::srgb(0.95, 0.76, 0.29);
pub const BONE_SELECTED: Color = Color::srgb(0.31, 0.55, 1.0);
pub const BONE_HOVERED: Color = Color::srgb(1.0, 0.88, 0.55);

/// A bone's colour: its side's, unless it is selected or under the cursor.
pub fn bone_colour(name: &str, selected: bool, hovered: bool) -> Color {
    if selected {
        return BONE_SELECTED;
    }
    if hovered {
        return BONE_HOVERED;
    }
    match bone_side(name) {
        BoneSide::Left => BONE_LEFT,
        BoneSide::Right => BONE_RIGHT,
        BoneSide::Centre => BONE_CENTRE,
    }
}

/// The ring drawn around a bone, a control to grab: a quarter of the way
/// along it, a fifth of its length across, and never so big on screen that a
/// short bone's ring swallows its neighbours. `camera_distance` is how far the
/// bone's joint is from the camera.
pub fn ring_radius(length: f32, camera_distance: f32) -> f32 {
    (length * 0.18).clamp(1e-4, (camera_distance * 0.03).max(1e-4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sides_follow_the_naming_conventions() {
        assert_eq!(bone_side("mixamorig_leftforearm"), BoneSide::Left);
        assert_eq!(bone_side("mixamorig:RightHand"), BoneSide::Right);
        assert_eq!(bone_side("arm.L"), BoneSide::Left);
        assert_eq!(bone_side("face_lid_upper_r"), BoneSide::Right);
        assert_eq!(bone_side("mixamorig:Spine"), BoneSide::Centre);
    }
}
