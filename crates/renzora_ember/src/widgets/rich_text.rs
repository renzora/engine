//! Rich text — multiple colored runs on one line.

use bevy::prelude::*;

use crate::font::ui_font;
use crate::theme::{rgb, TEXT_PRIMARY};

/// A line of text made of colored `(text, color)` runs.
pub fn rich_text(commands: &mut Commands, font: &Handle<Font>, runs: &[(&str, (u8, u8, u8))]) -> Entity {
    let root = commands
        .spawn((
            Text::new(""),
            ui_font(font, 13.0),
            TextColor(rgb(TEXT_PRIMARY)),
            Name::new("rich-text"),
        ))
        .id();
    let spans: Vec<Entity> = runs
        .iter()
        .map(|(s, color)| {
            commands
                .spawn((
                    TextSpan::new(s.to_string()),
                    ui_font(font, 13.0),
                    TextColor(rgb(*color)),
                ))
                .id()
        })
        .collect();
    commands.entity(root).add_children(&spans);
    root
}
