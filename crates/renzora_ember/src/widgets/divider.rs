//! Divider — a 1px horizontal rule.

use bevy::prelude::*;

use crate::theme::rgb;

/// A 1px horizontal divider.
pub fn divider(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(rgb((48, 48, 58))),
            Name::new("divider"),
        ))
        .id()
}
