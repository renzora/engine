//! Segmented control — a row of buttons, one selected.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::*;

#[derive(Component)]
pub(crate) struct EmberSegment {
    group: Entity,
    value: usize,
}

/// A segmented control (a row of buttons, one selected). Returns the container.
pub fn segmented(
    commands: &mut Commands,
    font: &Handle<Font>,
    options: &[&str],
    selected: usize,
) -> Entity {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb((40, 40, 48))),
            Name::new("segmented"),
        ))
        .id();
    let mut kids = Vec::new();
    for (i, label) in options.iter().enumerate() {
        let on = i == selected;
        let seg = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(if on {
                    rgb(accent())
                } else {
                    Color::NONE
                }),
                Interaction::default(),
                EmberSegment { group, value: i },
                Styled::with_state(
                    Role::Segment,
                    if on {
                        WidgetState::Active
                    } else {
                        WidgetState::Normal
                    },
                ),
                renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("segment"),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(*label),
                    ui_font(font, 12.0),
                    TextColor(rgb(text_primary())),
                ));
            })
            .id();
        kids.push(seg);
    }
    commands.entity(group).add_children(&kids);
    group
}

pub(crate) fn segmented_interact(
    pressed: Query<(&Interaction, &EmberSegment), Changed<Interaction>>,
    mut segments: Query<(&EmberSegment, &mut Styled)>,
) {
    let mut chosen: Option<(Entity, usize)> = None;
    for (interaction, s) in &pressed {
        if *interaction == Interaction::Pressed {
            chosen = Some((s.group, s.value));
            break;
        }
    }
    let Some((group, value)) = chosen else {
        return;
    };
    for (s, mut styled) in &mut segments {
        if s.group != group {
            continue;
        }
        styled.state = if s.value == value {
            WidgetState::Active
        } else {
            WidgetState::Normal
        };
    }
}
