//! List group — a vertical list of single-select rows.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::{rgb, TAB_ACTIVE_BG, TEXT_PRIMARY};

#[derive(Component)]
pub(crate) struct EmberListItem {
    group: Entity,
}

/// A vertical list group of selectable rows (`items`); `active` starts selected.
pub fn list_group(
    commands: &mut Commands,
    font: &Handle<Font>,
    items: &[&str],
    active: usize,
) -> Entity {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                min_width: Val::Px(160.0),
                ..default()
            },
            Name::new("list-group"),
        ))
        .id();
    let mut kids = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let on = i == active;
        let row = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(if on {
                    rgb(TAB_ACTIVE_BG)
                } else {
                    Color::NONE
                }),
                Interaction::default(),
                EmberListItem { group },
                Styled::with_state(
                    Role::Segment,
                    if on {
                        WidgetState::Active
                    } else {
                        WidgetState::Normal
                    },
                ),
                renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("list-item"),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(*item),
                    ui_font(font, 12.0),
                    TextColor(rgb(TEXT_PRIMARY)),
                ));
            })
            .id();
        kids.push(row);
    }
    commands.entity(group).add_children(&kids);
    group
}

pub(crate) fn list_select(
    pressed: Query<(Entity, &Interaction, &EmberListItem), Changed<Interaction>>,
    items: Query<(Entity, &EmberListItem)>,
    mut styles: Query<&mut Styled>,
) {
    let mut chosen: Option<(Entity, Entity)> = None;
    for (entity, interaction, item) in &pressed {
        if *interaction == Interaction::Pressed {
            chosen = Some((item.group, entity));
            break;
        }
    }
    let Some((group, picked)) = chosen else {
        return;
    };
    for (entity, item) in &items {
        if item.group != group {
            continue;
        }
        if let Ok(mut styled) = styles.get_mut(entity) {
            styled.state = if entity == picked {
                WidgetState::Active
            } else {
                WidgetState::Normal
            };
        }
    }
}
