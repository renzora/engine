//! Pagination — numbered page buttons, one active.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::{rgb, ACCENT_BLUE, TAB_ACTIVE_BG, TEXT_PRIMARY};

#[derive(Component)]
pub(crate) struct EmberPage {
    group: Entity,
}

/// A pagination control: numbered page buttons, one active.
pub fn pagination(commands: &mut Commands, font: &Handle<Font>, pages: usize, active: usize) -> Entity {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("pagination"),
        ))
        .id();
    let mut kids = Vec::new();
    for i in 0..pages {
        let on = i == active;
        let page = commands
            .spawn((
                Node {
                    min_width: Val::Px(26.0),
                    height: Val::Px(26.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(if on {
                    rgb(ACCENT_BLUE)
                } else {
                    rgb(TAB_ACTIVE_BG)
                }),
                Interaction::default(),
                EmberPage { group },
                Styled::with_state(
                    Role::Button,
                    if on {
                        WidgetState::Active
                    } else {
                        WidgetState::Normal
                    },
                ),
                renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("page"),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(format!("{}", i + 1)),
                    ui_font(font, 12.0),
                    TextColor(rgb(TEXT_PRIMARY)),
                ));
            })
            .id();
        kids.push(page);
    }
    commands.entity(group).add_children(&kids);
    group
}

pub(crate) fn pagination_select(
    pressed: Query<(Entity, &Interaction, &EmberPage), Changed<Interaction>>,
    pages: Query<(Entity, &EmberPage)>,
    mut styles: Query<&mut Styled>,
) {
    let mut chosen: Option<(Entity, Entity)> = None;
    for (entity, interaction, page) in &pressed {
        if *interaction == Interaction::Pressed {
            chosen = Some((page.group, entity));
            break;
        }
    }
    let Some((group, picked)) = chosen else {
        return;
    };
    for (entity, page) in &pages {
        if page.group != group {
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
