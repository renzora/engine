//! Vector edits — colored drag-value fields (X/Y/Z/W).

use bevy::prelude::*;

use super::common::hstack;
use super::drag_value::drag_value;

const X: (u8, u8, u8) = (224, 110, 110);
const Y: (u8, u8, u8) = (130, 200, 130);
const Z: (u8, u8, u8) = (120, 150, 240);
const W: (u8, u8, u8) = (200, 180, 90);

/// Two colored drag-value fields (X/Y).
pub fn vec2_edit(commands: &mut Commands, font: &bevy::text::FontSource, x: f32, y: f32) -> Entity {
    let fields = [
        drag_value(commands, font, "X", X, x, 0.05),
        drag_value(commands, font, "Y", Y, y, 0.05),
    ];
    hstack(commands, 6.0, &fields)
}

/// Three colored drag-value fields (X/Y/Z) for editing a vector.
pub fn vec3_edit(commands: &mut Commands, font: &bevy::text::FontSource, x: f32, y: f32, z: f32) -> Entity {
    let fields = [
        drag_value(commands, font, "X", X, x, 0.05),
        drag_value(commands, font, "Y", Y, y, 0.05),
        drag_value(commands, font, "Z", Z, z, 0.05),
    ];
    hstack(commands, 6.0, &fields)
}

/// Four colored drag-value fields (X/Y/Z/W) — vec4 / quaternion.
pub fn vec4_edit(commands: &mut Commands, font: &bevy::text::FontSource, x: f32, y: f32, z: f32, w: f32) -> Entity {
    let fields = [
        drag_value(commands, font, "X", X, x, 0.05),
        drag_value(commands, font, "Y", Y, y, 0.05),
        drag_value(commands, font, "Z", Z, z, 0.05),
        drag_value(commands, font, "W", W, w, 0.05),
    ];
    hstack(commands, 6.0, &fields)
}
