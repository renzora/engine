//! Input group — a static addon prefix joined to a text input.

use bevy::prelude::*;

use crate::theme::{rgb, HEADER_BG, TEXT_MUTED};

use super::common::text_node;
use super::text_input::text_input;

/// An input group: a static addon (prefix) joined to a text input.
pub fn input_group(commands: &mut Commands, font: &Handle<Font>, addon: &str, placeholder: &str) -> Entity {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("input-group"),
        ))
        .id();
    let addon_box = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::new(
                    Val::Px(4.0),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(4.0),
                ),
                ..default()
            },
            BackgroundColor(rgb(HEADER_BG)),
            BorderColor::all(rgb((70, 70, 82))),
            Name::new("input-addon"),
        ))
        .id();
    let addon_text = text_node(commands, font, addon, 12.0, TEXT_MUTED);
    commands.entity(addon_box).add_child(addon_text);
    let input = text_input(commands, font, placeholder, "");
    commands.entity(group).add_children(&[addon_box, input]);
    group
}
