//! Textarea — a multi-line text input (reuses the text-input systems).

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled};
use crate::theme::*;

use super::text_input::{caret, EmberTextInput};

/// A multi-line text area (reuses the text-input focus/typing systems; Enter
/// inserts a newline).
pub fn textarea(commands: &mut Commands, font: &bevy::text::FontSource, placeholder: &str, value: &str) -> Entity {
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
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Styled::new(Role::Input),
            Interaction::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Text),
            Name::new("textarea"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(if empty { placeholder } else { value }),
            ui_font(font, 12.0),
            TextColor(rgb(if empty { text_muted() } else { text_primary() })),
        ))
        .id();
    let car = caret(commands);
    commands.entity(box_e).insert(EmberTextInput {
        value: value.to_string(),
        focused: false,
        text_entity: text,
        placeholder: placeholder.to_string(),
        caret: car,
        password: false,
        select_all: false,
        // The multi-line textarea doesn't use single-line caret positioning
        // (no `SingleLineInput` marker), so these are inert for it.
        caret_index: value.chars().count(),
        advance: 6.0,
        offsets: Vec::new(),
        sel_anchor: None,
    });
    commands.entity(box_e).add_children(&[text, car]);
    box_e
}
