//! Card — a themed container with a heading + body.

use bevy::prelude::*;

use crate::style::{Role, Styled};
use crate::theme::*;

use super::common::text_node;

/// A titled card (themed `Card` container with a heading + body).
pub fn card(commands: &mut Commands, font: &bevy::text::FontSource, title: &str, body: &str) -> Entity {
    let c = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                min_width: Val::Px(180.0),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            Styled::new(Role::Card),
            Name::new("card"),
        ))
        .id();
    let t = text_node(commands, font, title, 13.0, text_primary());
    let b = text_node(commands, font, body, 12.0, text_muted());
    commands.entity(c).add_children(&[t, b]);
    c
}
