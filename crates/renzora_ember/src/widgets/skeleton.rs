//! Skeleton — a loading placeholder block.

use bevy::prelude::*;

use crate::theme::rgb;

/// A skeleton placeholder block (loading state; shimmer animation comes later).
pub fn skeleton(commands: &mut Commands, width: f32, height: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((48, 48, 58))),
            Name::new("skeleton"),
        ))
        .id()
}
