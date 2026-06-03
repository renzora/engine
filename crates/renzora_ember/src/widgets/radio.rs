//! Radio group — single-select ring options.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::theme::*;

#[derive(Component)]
pub(crate) struct EmberRadio {
    group: Entity,
    value: usize,
    dot: Entity,
}

/// A horizontal radio group; returns the group container. Exactly one option is
/// selected at a time.
pub fn radio_group(
    commands: &mut Commands,
    font: &Handle<Font>,
    options: &[&str],
    selected: usize,
) -> Entity {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                ..default()
            },
            Name::new("radio-group"),
        ))
        .id();
    let mut kids = Vec::new();
    for (i, label) in options.iter().enumerate() {
        let on = i == selected;
        let opt = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    ..default()
                },
                Interaction::default(),
                renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("radio"),
            ))
            .id();
        let ring = commands
            .spawn((
                Node {
                    width: Val::Px(16.0),
                    height: Val::Px(16.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BorderColor::all(rgb((92, 92, 104))),
                bevy::ui::FocusPolicy::Pass,
                Name::new("radio-ring"),
            ))
            .id();
        let dot = commands
            .spawn((
                Node {
                    width: Val::Px(8.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    display: if on { Display::Flex } else { Display::None },
                    ..default()
                },
                BackgroundColor(rgb(accent())),
                bevy::ui::FocusPolicy::Pass,
                Name::new("radio-dot"),
            ))
            .id();
        let text = commands
            .spawn((
                Text::new(*label),
                ui_font(font, 12.0),
                TextColor(rgb(text_primary())),
            ))
            .id();
        commands.entity(ring).add_child(dot);
        commands.entity(opt).insert(EmberRadio {
            group,
            value: i,
            dot,
        });
        commands.entity(opt).add_children(&[ring, text]);
        kids.push(opt);
    }
    commands.entity(group).add_children(&kids);
    group
}

pub(crate) fn radio_interact(
    pressed: Query<(&Interaction, &EmberRadio), Changed<Interaction>>,
    radios: Query<&EmberRadio>,
    mut nodes: Query<&mut Node>,
) {
    let mut chosen: Option<(Entity, usize)> = None;
    for (interaction, r) in &pressed {
        if *interaction == Interaction::Pressed {
            chosen = Some((r.group, r.value));
            break;
        }
    }
    let Some((group, value)) = chosen else {
        return;
    };
    for r in &radios {
        if r.group != group {
            continue;
        }
        if let Ok(mut n) = nodes.get_mut(r.dot) {
            n.display = if r.value == value {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}
