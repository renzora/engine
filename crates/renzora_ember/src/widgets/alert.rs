//! Alert — an inline message box (themed container + tone icon + title/body).

use bevy::prelude::*;

use crate::font::{icon_text, EmberFonts};
use crate::style::{Role, Styled};
use crate::theme::*;

use super::common::text_node;
use super::tone::Tone;

/// An inline alert box (themed container + tone icon + title/body).
pub fn alert(
    commands: &mut Commands,
    fonts: &EmberFonts,
    tone: Tone,
    title: &str,
    body: &str,
) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::FlexStart,
                min_width: Val::Px(240.0),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Styled::new(Role::Alert),
            Name::new("alert"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, tone.icon(), tone.color(), 16.0);
    let col = commands
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        },))
        .id();
    let t = text_node(commands, &fonts.ui, title, 13.0, text_primary());
    let b = text_node(commands, &fonts.ui, body, 12.0, text_muted());
    commands.entity(col).add_children(&[t, b]);
    commands.entity(box_e).add_children(&[icon, col]);
    box_e
}
