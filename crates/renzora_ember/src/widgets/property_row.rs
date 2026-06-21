//! Property row — an inspector label + right-aligned control.

use bevy::prelude::*;

use crate::theme::*;

use super::common::text_node;

/// An inspector property row: a muted label on the left, a control pushed to
/// the right.
pub fn property_row(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    label: &str,
    control: Entity,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(10.0),
                ..default()
            },
            Name::new("property-row"),
        ))
        .id();
    let lbl = text_node(commands, font, label, 12.0, text_muted());
    commands.entity(row).add_children(&[lbl, control]);
    row
}
