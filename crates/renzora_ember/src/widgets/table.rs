//! Table — a header row over data rows (equal-width columns).

use bevy::prelude::*;

use crate::theme::{rgb, HEADER_BG, TEXT_MUTED, TEXT_PRIMARY};

use super::common::text_node;

fn table_row(commands: &mut Commands, font: &Handle<Font>, cells: &[&str], header: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BackgroundColor(if header { rgb(HEADER_BG) } else { Color::NONE }),
            Name::new("table-row"),
        ))
        .id();
    let cells: Vec<Entity> = cells
        .iter()
        .map(|c| {
            let cell = commands
                .spawn((Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                    ..default()
                },))
                .id();
            let t = text_node(
                commands,
                font,
                c,
                12.0,
                if header { TEXT_PRIMARY } else { TEXT_MUTED },
            );
            commands.entity(cell).add_child(t);
            cell
        })
        .collect();
    commands.entity(row).add_children(&cells);
    row
}

/// A simple data table: a header row over data rows (equal-width columns).
pub fn table(commands: &mut Commands, font: &Handle<Font>, headers: &[&str], rows: &[&[&str]]) -> Entity {
    let t = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(240.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb((30, 30, 38))),
            BorderColor::all(rgb((48, 48, 58))),
            Name::new("table"),
        ))
        .id();
    let mut kids = vec![table_row(commands, font, headers, true)];
    for row in rows {
        kids.push(table_row(commands, font, row, false));
    }
    commands.entity(t).add_children(&kids);
    t
}
