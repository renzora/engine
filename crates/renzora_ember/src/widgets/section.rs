//! Section — a collapsible group: a colored header bar (caret, accent icon,
//! title) over a padded body you fill. Click the header to toggle. The editor's
//! panels (settings, inspector, …) compose this instead of hand-rolling their own
//! section headers, so they share one themed look and collapse behavior.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use crate::theme::{rgb, section_bg, text_muted, text_primary};

pub(crate) struct SectionPlugin;

impl Plugin for SectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, section_toggle);
    }
}

/// On a section header: its open state + the body/caret it drives.
#[derive(Component)]
pub struct Section {
    open: bool,
    body: Entity,
    caret: Entity,
}

/// Build a collapsible section with a colored header (caret + `accent`-colored
/// `icon` + `title`) over a padded body. Returns `(root, body)` — add content to
/// `body`. Use [`section_with_header`] when you need to add header affordances
/// (an enable toggle, remove button, …).
pub fn section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    title: &str,
    accent: (u8, u8, u8),
) -> (Entity, Entity) {
    let (root, _header, body) = section_with_header(commands, fonts, icon, title, accent);
    (root, body)
}

/// Like [`section`] but also returns the `header` entity, so callers can add
/// trailing controls (enable toggle, remove, lock) to it.
pub fn section_with_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    title: &str,
    accent: (u8, u8, u8),
) -> (Entity, Entity, Entity) {
    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect {
                    left: Val::Px(8.0),
                    top: Val::Px(4.0),
                    bottom: Val::Px(4.0),
                    ..default()
                },
                ..default()
            },
            Name::new("section-body"),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 12.0);
    let ico = icon_text(commands, &fonts.phosphor, icon, accent, 14.0);
    let heading = commands
        .spawn((
            Text::new(title),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            Interaction::default(),
            Section {
                open: true,
                body,
                caret,
            },
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("section-header"),
        ))
        .id();
    commands.entity(header).add_children(&[caret, ico, heading]);

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            Name::new("section"),
        ))
        .id();
    commands.entity(root).add_children(&[header, body]);
    (root, header, body)
}

/// Click a section header → toggle its body + flip the caret.
pub(crate) fn section_toggle(
    mut headers: Query<(&Interaction, &mut Section), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, mut sec) in &mut headers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        sec.open = !sec.open;
        if let Ok(mut n) = nodes.get_mut(sec.body) {
            n.display = if sec.open { Display::Flex } else { Display::None };
        }
        if let Ok(mut t) = texts.get_mut(sec.caret) {
            if let Some(g) = icon_glyph(if sec.open { "caret-down" } else { "caret-right" }) {
                t.0 = g.to_string();
            }
        }
    }
}
