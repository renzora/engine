//! Grid — a uniform grid of cells.

use bevy::prelude::*;

use crate::font::ui_font;
use crate::theme::*;

/// A uniform grid of `count` cells across `columns` flexible columns.
pub fn grid(commands: &mut Commands, font: &bevy::text::FontSource, count: usize, columns: usize) -> Entity {
    let g = commands
        .spawn((
            Node {
                display: Display::Grid,
                grid_template_columns: vec![bevy::ui::RepeatedGridTrack::flex(columns as u16, 1.0)],
                row_gap: Val::Px(6.0),
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("grid"),
        ))
        .id();
    let cells: Vec<Entity> = (0..count)
        .map(|i| {
            commands
                .spawn((
                    Node {
                        height: Val::Px(40.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(card_bg())),
                    Name::new("grid-cell"),
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new(format!("{}", i + 1)),
                        ui_font(font, 12.0),
                        TextColor(rgb(text_muted())),
                    ));
                })
                .id()
        })
        .collect();
    commands.entity(g).add_children(&cells);
    g
}
