//! Spinner — an animated loading indicator (pulsing dots).

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::theme::{rgb, ACCENT_BLUE};

#[derive(Component)]
pub(crate) struct EmberSpinnerDot {
    phase: f32,
}

/// A small "•••" pulsing loading indicator.
pub fn spinner(commands: &mut Commands) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                ..default()
            },
            Name::new("spinner"),
        ))
        .id();
    let dots: Vec<Entity> = (0..3)
        .map(|i| {
            commands
                .spawn((
                    Node {
                        width: Val::Px(7.0),
                        height: Val::Px(7.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(ACCENT_BLUE)),
                    EmberSpinnerDot {
                        phase: i as f32 / 3.0,
                    },
                    Name::new("spinner-dot"),
                ))
                .id()
        })
        .collect();
    commands.entity(row).add_children(&dots);
    row
}

pub(crate) fn spinner_anim(time: Res<Time>, mut dots: Query<(&EmberSpinnerDot, &mut BackgroundColor)>) {
    let t = time.elapsed_secs();
    for (dot, mut bg) in &mut dots {
        let wave = 0.5 + 0.5 * (t * 5.0 - dot.phase * TAU).sin();
        let alpha = (0.3 + 0.7 * wave).clamp(0.0, 1.0);
        bg.0 = rgb(ACCENT_BLUE).with_alpha(alpha);
    }
}
