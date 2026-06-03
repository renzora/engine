//! Validation — a tone-colored input box with an icon + message below.

use bevy::prelude::*;

use crate::font::{icon_text, EmberFonts};
use crate::theme::*;

use super::common::text_node;
use super::tone::Tone;

/// A validation field: a tone-colored input box with an icon + message below.
pub fn validation(commands: &mut Commands, fonts: &EmberFonts, tone: Tone, value: &str, message: &str) -> Entity {
    let col = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            Name::new("validation"),
        ))
        .id();
    let box_e = commands
        .spawn((
            Node {
                min_width: Val::Px(200.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(tone.color())),
            Name::new("validation-input"),
        ))
        .id();
    let val = text_node(commands, &fonts.ui, value, 12.0, text_primary());
    commands.entity(box_e).add_child(val);
    let msg_row = commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        },))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, tone.icon(), tone.color(), 12.0);
    let msg = text_node(commands, &fonts.ui, message, 11.0, tone.color());
    commands.entity(msg_row).add_children(&[icon, msg]);
    commands.entity(col).add_children(&[box_e, msg_row]);
    col
}
