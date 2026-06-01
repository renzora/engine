//! Vec3 edit — three colored drag-value fields (X/Y/Z).

use bevy::prelude::*;

use super::common::hstack;
use super::drag_value::drag_value;

/// Three colored drag-value fields (X/Y/Z) for editing a vector.
pub fn vec3_edit(commands: &mut Commands, font: &Handle<Font>, x: f32, y: f32, z: f32) -> Entity {
    let fields = [
        drag_value(commands, font, "X", (224, 110, 110), x, 0.05),
        drag_value(commands, font, "Y", (130, 200, 130), y, 0.05),
        drag_value(commands, font, "Z", (120, 150, 240), z, 0.05),
    ];
    hstack(commands, 6.0, &fields)
}
