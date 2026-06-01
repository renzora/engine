//! Drag-value — a scrubbable numeric field (drag horizontally to change).

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::style::{Role, Styled};
use crate::theme::{rgb, TEXT_PRIMARY};

use super::common::{format_num, text_node};

#[derive(Component)]
pub(crate) struct EmberDragValue {
    value: f32,
    step: f32,
    text: Entity,
    last_x: Option<f32>,
}

/// A scrubbable numeric field. `axis` is an optional colored prefix (e.g. "X").
pub fn drag_value(
    commands: &mut Commands,
    font: &Handle<Font>,
    axis: &str,
    axis_color: (u8, u8, u8),
    value: f32,
    step: f32,
) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                min_width: Val::Px(58.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((28, 28, 34))),
            BorderColor::all(rgb((70, 70, 82))),
            Styled::new(Role::Input),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::EwResize),
            Name::new("drag-value"),
        ))
        .id();
    let text = text_node(commands, font, &format_num(value), 12.0, TEXT_PRIMARY);
    let mut kids = Vec::new();
    if !axis.is_empty() {
        kids.push(text_node(commands, font, axis, 11.0, axis_color));
    }
    kids.push(text);
    commands.entity(box_e).insert(EmberDragValue {
        value,
        step,
        text,
        last_x: None,
    });
    commands.entity(box_e).add_children(&kids);
    box_e
}

pub(crate) fn drag_value_drag(
    windows: Query<&Window>,
    mut values: Query<(&Interaction, &mut EmberDragValue)>,
    mut texts: Query<&mut Text>,
) {
    let cursor_x = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|p| p.x);
    for (interaction, mut dv) in &mut values {
        if *interaction == Interaction::Pressed {
            if let (Some(cx), Some(last)) = (cursor_x, dv.last_x) {
                let delta = cx - last;
                if delta != 0.0 {
                    dv.value += delta * dv.step;
                    let (t, v) = (dv.text, dv.value);
                    if let Ok(mut text) = texts.get_mut(t) {
                        *text = Text::new(format_num(v));
                    }
                }
            }
            dv.last_x = cursor_x;
        } else if dv.last_x.is_some() {
            dv.last_x = None;
        }
    }
}
