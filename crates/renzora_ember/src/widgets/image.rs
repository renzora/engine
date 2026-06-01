//! Image — a fixed-size image node.

use bevy::prelude::*;

/// An image node at a fixed size (caller supplies the texture handle).
pub fn image(commands: &mut Commands, texture: Handle<Image>, width: f32, height: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            ImageNode::new(texture),
            Name::new("image"),
        ))
        .id()
}
