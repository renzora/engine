//! Typography — headings, body, caption, label, link, inline code.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::theme::{rgb, ACCENT_BLUE, TEXT_MUTED, TEXT_PRIMARY};

use super::common::text_node;

/// Display heading, level 1 (largest).
pub fn h1(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 26.0, TEXT_PRIMARY)
}
/// Heading, level 2.
pub fn h2(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 21.0, TEXT_PRIMARY)
}
/// Heading, level 3.
pub fn h3(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 17.0, TEXT_PRIMARY)
}
/// Heading, level 4 (smallest).
pub fn h4(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 14.0, TEXT_PRIMARY)
}
/// Body paragraph text.
pub fn paragraph(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 13.0, TEXT_PRIMARY)
}
/// Small, muted caption.
pub fn caption(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 11.0, TEXT_MUTED)
}
/// A muted form/field label.
pub fn label(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 12.0, TEXT_MUTED)
}

/// An accent-colored hyperlink (pointer cursor; click handling is the caller's).
pub fn link(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(font, 12.0),
            TextColor(rgb(ACCENT_BLUE)),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("link"),
        ))
        .id()
}

/// Inline code — a subtle chip around monospaced-looking text.
pub fn code(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((28, 28, 34))),
            Name::new("code"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(text),
                ui_font(font, 12.0),
                TextColor(rgb((200, 210, 235))),
            ));
        })
        .id()
}
