//! Avatar — initials on a colored disc.

use bevy::prelude::*;

use crate::font::ui_font;
use crate::theme::rgb;

/// A circular avatar showing initials on a colored disc.
pub fn avatar(commands: &mut Commands, font: &bevy::text::FontSource, initials: &str, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(rgb(color)),
            Name::new("avatar"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(initials),
                ui_font(font, 12.0),
                TextColor(rgb((255, 255, 255))),
            ));
        })
        .id()
}
