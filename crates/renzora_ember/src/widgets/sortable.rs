//! Sortable list — drag a row by its handle to reorder it within the list.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, EmberFonts};
use crate::theme::*;

#[derive(Component)]
pub(crate) struct EmberSortable;

#[derive(Component)]
pub(crate) struct EmberSortItem {
    list: Entity,
}

/// A vertical list whose rows can be drag-reordered (each row has a ⋮⋮ handle).
pub fn sortable_list(commands: &mut Commands, fonts: &EmberFonts, items: &[&str]) -> Entity {
    let list = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                min_width: Val::Px(180.0),
                ..default()
            },
            RelativeCursorPosition::default(),
            EmberSortable,
            Name::new("sortable"),
        ))
        .id();
    let rows: Vec<Entity> = items
        .iter()
        .map(|item| {
            let row = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(tab_active())),
                    Interaction::default(),
                    EmberSortItem { list },
                    crate::cursor_icon::HoverCursor(SystemCursorIcon::Grab),
                    Name::new("sort-item"),
                ))
                .id();
            let handle = icon_text(commands, &fonts.phosphor, "dots-six-vertical", text_muted(), 14.0);
            let text = commands
                .spawn((
                    Text::new(*item),
                    crate::font::ui_font(&fonts.ui, 12.0),
                    TextColor(rgb(text_primary())),
                ))
                .id();
            commands.entity(row).add_children(&[handle, text]);
            row
        })
        .collect();
    commands.entity(list).add_children(&rows);
    list
}

pub(crate) fn sortable_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    mut dragging: Local<Option<(Entity, Entity)>>,
    rows: Query<(Entity, &Interaction, &EmberSortItem)>,
    lists: Query<&RelativeCursorPosition, With<EmberSortable>>,
    children_q: Query<&Children>,
    mut commands: Commands,
) {
    if dragging.is_none() {
        if mouse.just_pressed(MouseButton::Left) {
            for (e, interaction, item) in &rows {
                if *interaction == Interaction::Pressed {
                    *dragging = Some((e, item.list));
                    break;
                }
            }
        }
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        *dragging = None;
        return;
    }
    let Some((row, list)) = *dragging else {
        return;
    };
    let Ok(rcp) = lists.get(list) else {
        return;
    };
    let Some(nrm) = rcp.normalized else {
        return;
    };
    let Ok(kids) = children_q.get(list) else {
        return;
    };
    let n = kids.len();
    if n == 0 {
        return;
    }
    let target = (((nrm.y + 0.5) * n as f32).floor() as isize).clamp(0, n as isize - 1) as usize;
    let cur = kids.iter().position(|c| c == row);
    if Some(target) != cur {
        commands.entity(list).insert_children(target, &[row]);
    }
}
