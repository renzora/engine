//! Toast — a notification card (themed + tone icon + message + close ×).

use bevy::prelude::*;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::style::{Role, Styled};
use crate::theme::*;

use super::tone::Tone;

/// A toast notification (themed card + tone icon + message + close ×).
pub fn toast(commands: &mut Commands, fonts: &EmberFonts, tone: Tone, message: &str) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                min_width: Val::Px(220.0),
                ..default()
            },
            BackgroundColor(rgb((44, 44, 55))),
            BorderColor::all(rgb((64, 64, 78))),
            Styled::new(Role::Toast),
            Name::new("toast"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, tone.icon(), tone.color(), 14.0);
    let msg = commands
        .spawn((
            Text::new(message),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ))
        .id();
    let close = icon_text(commands, &fonts.phosphor, "x", text_muted(), 12.0);
    commands.entity(box_e).add_children(&[icon, msg, close]);
    box_e
}
