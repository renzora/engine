//! Hamburger menu — an icon button that toggles a popup menu.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, EmberFonts};
use crate::style::{Role, Styled};
use crate::theme::{rgb, TAB_ACTIVE_BG, TEXT_PRIMARY};

use super::button::EmberButton;
use super::menu::build_menu;

#[derive(Component)]
pub(crate) struct EmberHamburger {
    menu: Entity,
    open: bool,
}

/// A hamburger (☰) button that toggles a popup menu of `items`.
pub fn hamburger(commands: &mut Commands, fonts: &EmberFonts, items: &[&str]) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("hamburger"),
        ))
        .id();
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(30.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            Interaction::default(),
            EmberButton,
            Styled::new(Role::Button),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("hamburger-btn"),
        ))
        .id();
    let glyph = icon_text(commands, &fonts.phosphor, "list", TEXT_PRIMARY, 16.0);
    commands.entity(btn).add_child(glyph);
    let entries: Vec<(&str, &[&str])> = items.iter().map(|s| (*s, &[] as &[&str])).collect();
    let menu = build_menu(commands, fonts, &entries, true);
    commands
        .entity(btn)
        .insert(EmberHamburger { menu, open: false });
    commands.entity(wrap).add_children(&[btn, menu]);
    wrap
}

pub(crate) fn hamburger_toggle(
    mut buttons: Query<(&Interaction, &mut EmberHamburger), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut h) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        h.open = !h.open;
        if let Ok(mut n) = nodes.get_mut(h.menu) {
            n.display = if h.open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}
