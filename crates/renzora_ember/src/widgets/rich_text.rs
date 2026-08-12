//! Rich text — multiple colored runs in one wrapping text block.

use bevy::prelude::*;

use crate::font::ui_font;
use crate::theme::*;

/// A block of text made of colored `(text, color)` runs, at the default 13px.
pub fn rich_text(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    runs: &[(&str, (u8, u8, u8))],
) -> Entity {
    rich_text_sized(commands, font, runs, 13.0)
}

/// [`rich_text`] at an explicit font size, for callers whose surrounding type
/// scale isn't 13px — a run at the wrong size is obvious next to plain body text,
/// since the two sit on the same line.
pub fn rich_text_sized(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    runs: &[(&str, (u8, u8, u8))],
    size: f32,
) -> Entity {
    let root = commands
        .spawn((
            Text::new(""),
            ui_font(font, size),
            TextColor(rgb(text_primary())),
            Name::new("rich-text"),
        ))
        .id();
    let spans: Vec<Entity> = runs
        .iter()
        .map(|(s, color)| {
            commands
                .spawn((
                    TextSpan::new(s.to_string()),
                    ui_font(font, size),
                    TextColor(rgb(*color)),
                ))
                .id()
        })
        .collect();
    commands.entity(root).add_children(&spans);
    root
}
