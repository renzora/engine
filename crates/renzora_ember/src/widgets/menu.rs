//! Menu — a reusable popup list of items (with one level of submenus). Shared
//! by [`super::hamburger`] and [`super::context_menu`].

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::theme::*;

/// Clicking this row hides the root menu it belongs to.
#[derive(Component)]
pub(crate) struct EmberMenuClose {
    pub(crate) root: Entity,
}

/// Hovering this row reveals its submenu panel.
#[derive(Component)]
pub(crate) struct EmberSubmenu {
    sub: Entity,
}

/// Any hoverable menu row (for the hover highlight).
#[derive(Component)]
pub(crate) struct EmberMenuRow;

/// Build a hidden popup menu panel. Each item is `(label, submenu_labels)`; a
/// non-empty submenu renders a ▸ and a nested panel that opens on hover.
/// `below` positions the panel under its anchor (vs. at the anchor's origin).
pub(crate) fn build_menu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    items: &[(&str, &[&str])],
    below: bool,
) -> Entity {
    let menu = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: if below {
                    Val::Percent(100.0)
                } else {
                    Val::Px(0.0)
                },
                left: Val::Px(0.0),
                margin: UiRect::top(Val::Px(if below { 2.0 } else { 0.0 })),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                min_width: Val::Px(150.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(700),
            super::popup::OverlaySurface,
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("menu"),
        ))
        .id();
    let mut rows = Vec::new();
    for (label, submenu) in items {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(14.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    position_type: PositionType::Relative,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                EmberMenuRow,
                crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("menu-item"),
            ))
            .id();
        let lbl = commands
            .spawn((
                Text::new(*label),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_primary())),
            ))
            .id();
        let mut row_kids = vec![lbl];
        if submenu.is_empty() {
            commands.entity(row).insert(EmberMenuClose { root: menu });
        } else {
            let arrow = icon_text(commands, &fonts.phosphor, "caret-right", text_muted(), 10.0);
            row_kids.push(arrow);
            let sub = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(100.0),
                        top: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(3.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        min_width: Val::Px(130.0),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(rgb(popup_bg())),
                    BorderColor::all(rgb(border())),
                    bevy::ui::RelativeCursorPosition::default(),
                    GlobalZIndex(701),
                    Name::new("submenu"),
                ))
                .id();
            let sub_rows: Vec<Entity> = submenu
                .iter()
                .map(|s| {
                    commands
                        .spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::NONE),
                            Interaction::default(),
                            EmberMenuRow,
                            EmberMenuClose { root: menu },
                            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                            Name::new("submenu-item"),
                        ))
                        .with_children(|p| {
                            p.spawn((
                                Text::new(*s),
                                ui_font(&fonts.ui, 12.0),
                                TextColor(rgb(text_primary())),
                            ));
                        })
                        .id()
                })
                .collect();
            commands.entity(sub).add_children(&sub_rows);
            commands.entity(row).insert(EmberSubmenu { sub });
            row_kids.push(sub);
        }
        commands.entity(row).add_children(&row_kids);
        rows.push(row);
    }
    commands.entity(menu).add_children(&rows);
    menu
}

pub(crate) fn menu_hover(
    mut rows: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<EmberMenuRow>)>,
) {
    for (interaction, mut bg) in &mut rows {
        bg.0 = match *interaction {
            Interaction::Hovered | Interaction::Pressed => rgb(section_bg()),
            Interaction::None => Color::NONE,
        };
    }
}

pub(crate) fn submenu_hover(
    parents: Query<(&Interaction, &EmberSubmenu)>,
    subs: Query<&bevy::ui::RelativeCursorPosition>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, parent) in &parents {
        let sub_over = subs
            .get(parent.sub)
            .map(|r| r.cursor_over)
            .unwrap_or(false);
        let show = *interaction != Interaction::None || sub_over;
        if let Ok(mut n) = nodes.get_mut(parent.sub) {
            n.display = if show { Display::Flex } else { Display::None };
        }
    }
}

pub(crate) fn menu_item_close(
    pressed: Query<(&Interaction, &EmberMenuClose), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, close) in &pressed {
        if *interaction == Interaction::Pressed {
            if let Ok(mut n) = nodes.get_mut(close.root) {
                n.display = Display::None;
            }
        }
    }
}
