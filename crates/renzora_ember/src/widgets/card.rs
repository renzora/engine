//! Card — a themed container with a heading + body.

use bevy::prelude::*;

use crate::style::{Role, Styled};
use crate::theme::{rgb, PANEL_BG, TEXT_MUTED, TEXT_PRIMARY};

use super::common::text_node;

/// A titled card (themed `Card` container with a heading + body).
pub fn card(commands: &mut Commands, font: &Handle<Font>, title: &str, body: &str) -> Entity {
    let c = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                min_width: Val::Px(180.0),
                ..default()
            },
            BackgroundColor(rgb(PANEL_BG)),
            BorderColor::all(rgb((48, 48, 58))),
            Styled::new(Role::Card),
            Name::new("card"),
        ))
        .id();
    let t = text_node(commands, font, title, 13.0, TEXT_PRIMARY);
    let b = text_node(commands, font, body, 12.0, TEXT_MUTED);
    commands.entity(c).add_children(&[t, b]);
    c
}
