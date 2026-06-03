//! Chip — a removable tag (click the × to despawn it).

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, EmberFonts};
use crate::theme::*;

use super::common::text_node;

#[derive(Component)]
pub(crate) struct EmberChipClose {
    chip: Entity,
}

/// A removable tag chip (click the × to despawn it).
pub fn chip(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    let c = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(rgb((50, 50, 62))),
            Name::new("chip"),
        ))
        .id();
    let label = text_node(commands, &fonts.ui, text, 11.0, text_primary());
    let x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 10.0);
    let close = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            EmberChipClose { chip: c },
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("chip-close"),
        ))
        .id();
    commands.entity(close).add_child(x);
    commands.entity(c).add_children(&[label, close]);
    c
}

pub(crate) fn chip_close(
    pressed: Query<(&Interaction, &EmberChipClose), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (interaction, close) in &pressed {
        if *interaction == Interaction::Pressed {
            commands.entity(close.chip).despawn();
        }
    }
}
