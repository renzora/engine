//! Badge — a small semantic-tone pill.

use bevy::prelude::*;

use crate::font::ui_font;
use crate::theme::rgb;

use super::tone::Tone;

/// A small pill badge in a semantic tone.
pub fn badge(commands: &mut Commands, font: &bevy::text::FontSource, text: &str, tone: Tone) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(rgb(tone.color())),
            Name::new("badge"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(text),
                ui_font(font, 11.0),
                TextColor(rgb((255, 255, 255))),
            ));
        })
        .id()
}
