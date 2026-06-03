//! Progress — a determinate progress bar.

use bevy::prelude::*;

use crate::theme::*;

/// A determinate progress bar (`value` 0..1).
pub fn progress(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let track = commands
        .spawn((
            Node {
                width: Val::Px(180.0),
                height: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb((40, 40, 48))),
            Name::new("progress"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(v * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            Name::new("progress-fill"),
        ))
        .id();
    commands.entity(track).add_child(fill);
    track
}
