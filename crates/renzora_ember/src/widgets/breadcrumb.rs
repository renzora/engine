//! Breadcrumb — a path trail with the last segment highlighted.

use bevy::prelude::*;

use crate::font::{icon_text, EmberFonts};
use crate::theme::{TEXT_MUTED, TEXT_PRIMARY};

use super::common::text_node;

/// A breadcrumb trail (`segments` joined by ›); the last is highlighted.
pub fn breadcrumb(commands: &mut Commands, fonts: &EmberFonts, segments: &[&str]) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                ..default()
            },
            Name::new("breadcrumb"),
        ))
        .id();
    let mut kids = Vec::new();
    let last = segments.len().saturating_sub(1);
    for (i, seg) in segments.iter().enumerate() {
        let color = if i == last { TEXT_PRIMARY } else { TEXT_MUTED };
        kids.push(text_node(commands, &fonts.ui, seg, 12.0, color));
        if i != last {
            kids.push(icon_text(commands, &fonts.phosphor, "caret-right", TEXT_MUTED, 10.0));
        }
    }
    commands.entity(row).add_children(&kids);
    row
}
