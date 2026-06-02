//! A self-contained collapsible section: a clickable caret (+ optional icon) +
//! title header over a body you fill. Manages its own open/closed state — click
//! the header to toggle. The body's reactive content is independent of this.

use bevy::prelude::*;

use crate::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use crate::theme::{rgb, TEXT_MUTED, TEXT_PRIMARY};

pub(crate) struct CollapsiblePlugin;

impl Plugin for CollapsiblePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, collapsible_toggle);
    }
}

/// On a collapsible header: its open state + the body/caret it drives.
#[derive(Component)]
struct Collapsible {
    open: bool,
    body: Entity,
    caret: Entity,
}

/// Build a collapsible section. Returns `(root, body)` — add your content to
/// `body`. `icon` is an optional Phosphor name shown before the title.
pub fn collapsible(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: Option<&str>,
    title: &str,
    default_open: bool,
) -> (Entity, Entity) {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();

    let header = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("collapsible-header"),
        ))
        .id();
    let caret = icon_text(
        commands,
        &fonts.phosphor,
        if default_open { "caret-down" } else { "caret-right" },
        TEXT_MUTED,
        12.0,
    );
    let mut header_kids = vec![caret];
    if let Some(name) = icon {
        header_kids.push(icon_text(commands, &fonts.phosphor, name, TEXT_PRIMARY, 12.0));
    }
    let title_e = commands
        .spawn((
            Text::new(title),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    header_kids.push(title_e);
    commands.entity(header).add_children(&header_kids);

    let body = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(2.0),
            margin: UiRect::left(Val::Px(4.0)),
            display: if default_open {
                Display::Flex
            } else {
                Display::None
            },
            ..default()
        })
        .id();

    commands.entity(header).insert(Collapsible {
        open: default_open,
        body,
        caret,
    });
    commands.entity(root).add_children(&[header, body]);
    (root, body)
}

fn collapsible_toggle(
    mut q: Query<(&Interaction, &mut Collapsible), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, mut c) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        c.open = !c.open;
        if let Ok(mut n) = nodes.get_mut(c.body) {
            n.display = if c.open {
                Display::Flex
            } else {
                Display::None
            };
        }
        if let Ok(mut t) = texts.get_mut(c.caret) {
            let g = icon_glyph(if c.open { "caret-down" } else { "caret-right" }).unwrap_or(' ');
            t.0 = g.to_string();
        }
    }
}
