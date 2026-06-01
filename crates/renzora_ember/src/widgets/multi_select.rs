//! Multi-select — a list where each row toggles independently.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::{rgb, ACCENT_BLUE, TEXT_PRIMARY};

#[derive(Component)]
pub(crate) struct EmberMultiItem {
    selected: bool,
    mark: Entity,
}

/// A multi-select list: clicking a row toggles its own selected state.
/// `selected` gives the initial state per item (missing entries default off).
pub fn multi_select(commands: &mut Commands, font: &Handle<Font>, items: &[&str], selected: &[bool]) -> Entity {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                min_width: Val::Px(170.0),
                ..default()
            },
            Name::new("multi-select"),
        ))
        .id();
    let mut kids = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let on = selected.get(i).copied().unwrap_or(false);
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                Styled::with_state(
                    Role::Segment,
                    if on {
                        WidgetState::Active
                    } else {
                        WidgetState::Normal
                    },
                ),
                renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("multi-item"),
            ))
            .id();
        let box_e = commands
            .spawn((
                Node {
                    width: Val::Px(16.0),
                    height: Val::Px(16.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BorderColor::all(rgb((92, 92, 104))),
                Name::new("multi-box"),
            ))
            .id();
        let mark = commands
            .spawn((
                Node {
                    width: Val::Px(8.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    display: if on { Display::Flex } else { Display::None },
                    ..default()
                },
                BackgroundColor(rgb(ACCENT_BLUE)),
                Name::new("multi-mark"),
            ))
            .id();
        let text = commands
            .spawn((
                Text::new(*item),
                ui_font(font, 12.0),
                TextColor(rgb(TEXT_PRIMARY)),
            ))
            .id();
        commands.entity(box_e).add_child(mark);
        commands.entity(row).insert(EmberMultiItem { selected: on, mark });
        commands.entity(row).add_children(&[box_e, text]);
        kids.push(row);
    }
    commands.entity(group).add_children(&kids);
    group
}

pub(crate) fn multi_select_toggle(
    mut rows: Query<(&Interaction, &mut EmberMultiItem, &mut Styled), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut item, mut styled) in &mut rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        item.selected = !item.selected;
        styled.state = if item.selected {
            WidgetState::Active
        } else {
            WidgetState::Normal
        };
        if let Ok(mut n) = nodes.get_mut(item.mark) {
            n.display = if item.selected {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}
