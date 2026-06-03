//! Navbar — a brand on the left, nav links on the right.

use bevy::prelude::*;

use crate::font::EmberFonts;
use crate::theme::*;

use super::common::text_node;

/// A horizontal navbar: a brand on the left, nav links on the right.
pub fn navbar(commands: &mut Commands, fonts: &EmberFonts, brand: &str, links: &[&str]) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            Name::new("navbar"),
        ))
        .id();
    let brand_e = text_node(commands, &fonts.ui, brand, 14.0, text_primary());
    let link_row = commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(12.0),
            ..default()
        },))
        .id();
    let link_kids: Vec<Entity> = links
        .iter()
        .map(|l| text_node(commands, &fonts.ui, l, 12.0, text_muted()))
        .collect();
    commands.entity(link_row).add_children(&link_kids);
    commands.entity(bar).add_children(&[brand_e, link_row]);
    bar
}
