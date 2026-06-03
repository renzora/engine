//! Accordion — a collapsible titled section.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_hui::phosphor_map::icon_glyph;

use crate::font::{icon_text, EmberFonts};
use crate::theme::*;

use super::common::text_node;

#[derive(Component)]
pub(crate) struct EmberAccordion {
    body: Entity,
    caret: Entity,
    open: bool,
}

/// One collapsible accordion section: a clickable header over `content`.
pub fn accordion_section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    content: Entity,
    open: bool,
) -> Entity {
    let section = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("accordion-section"),
        ))
        .id();
    let header = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("accordion-header"),
        ))
        .id();
    let caret = icon_text(
        commands,
        &fonts.phosphor,
        if open { "caret-down" } else { "caret-right" },
        text_muted(),
        12.0,
    );
    let label = text_node(commands, &fonts.ui, title, 12.0, text_primary());
    commands.entity(header).add_children(&[caret, label]);
    let body = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                display: if open { Display::Flex } else { Display::None },
                ..default()
            },
            Name::new("accordion-body"),
        ))
        .id();
    commands.entity(body).add_child(content);
    commands
        .entity(header)
        .insert(EmberAccordion { body, caret, open });
    commands.entity(section).add_children(&[header, body]);
    section
}

pub(crate) fn accordion_toggle(
    mut headers: Query<(&Interaction, &mut EmberAccordion), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, mut acc) in &mut headers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        acc.open = !acc.open;
        if let Ok(mut n) = nodes.get_mut(acc.body) {
            n.display = if acc.open {
                Display::Flex
            } else {
                Display::None
            };
        }
        if let Ok(mut t) = texts.get_mut(acc.caret) {
            let glyph = icon_glyph(if acc.open { "caret-down" } else { "caret-right" })
                .unwrap_or('\u{E4C6}');
            *t = Text::new(glyph.to_string());
        }
    }
}
