//! Textarea — a multi-line text input (reuses the text-input systems).

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled};
use crate::theme::{rgb, TEXT_MUTED, TEXT_PRIMARY};

use super::text_input::EmberTextInput;

/// A multi-line text area (reuses the text-input focus/typing systems; Enter
/// inserts a newline).
pub fn textarea(commands: &mut Commands, font: &Handle<Font>, placeholder: &str, value: &str) -> Entity {
    let empty = value.is_empty();
    let box_e = commands
        .spawn((
            Node {
                min_width: Val::Px(200.0),
                min_height: Val::Px(64.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                align_items: AlignItems::FlexStart,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((28, 28, 34))),
            BorderColor::all(rgb((70, 70, 82))),
            Styled::new(Role::Input),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Text),
            Name::new("textarea"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(if empty { placeholder } else { value }),
            ui_font(font, 12.0),
            TextColor(rgb(if empty { TEXT_MUTED } else { TEXT_PRIMARY })),
        ))
        .id();
    commands.entity(box_e).insert(EmberTextInput {
        value: value.to_string(),
        focused: false,
        text_entity: text,
        placeholder: placeholder.to_string(),
    });
    commands.entity(box_e).add_child(text);
    box_e
}
