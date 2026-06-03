//! Floating label — a label floated above a filled input.

use bevy::prelude::*;

use crate::theme::*;

use super::common::text_node;
use super::text_input::text_input;

/// A floating-label field (label floated above a filled input).
pub fn floating_label(commands: &mut Commands, font: &Handle<Font>, label: &str, value: &str) -> Entity {
    let col = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Name::new("floating-label"),
        ))
        .id();
    let lbl = text_node(commands, font, label, 10.0, accent());
    let input = text_input(commands, font, "", value);
    commands.entity(col).add_children(&[lbl, input]);
    col
}
