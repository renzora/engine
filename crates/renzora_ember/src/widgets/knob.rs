//! Knob — a rotary control (drag vertically), indicator dot on the circumference.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::theme::{rgb, ACCENT_BLUE};

#[derive(Component)]
pub(crate) struct EmberKnob {
    value: f32,
    indicator: Entity,
}

/// Top-left offset of the knob indicator dot for a given value (270° sweep).
fn knob_offset(value: f32) -> (f32, f32) {
    let theta = (-135.0 + value * 270.0_f32).to_radians();
    let cx = 22.0 + 14.0 * theta.sin();
    let cy = 22.0 - 14.0 * theta.cos();
    (cx - 3.0, cy - 3.0)
}

/// A rotary knob (drag vertically to change `value` 0..1).
pub fn knob(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let body = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Px(44.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(22.0)),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(rgb((40, 40, 48))),
            BorderColor::all(rgb((70, 70, 82))),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::NsResize),
            Name::new("knob"),
        ))
        .id();
    let (lx, ty) = knob_offset(v);
    let indicator = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(lx),
                top: Val::Px(ty),
                width: Val::Px(6.0),
                height: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("knob-indicator"),
        ))
        .id();
    commands.entity(body).add_child(indicator);
    commands.entity(body).insert(EmberKnob { value: v, indicator });
    body
}

pub(crate) fn knob_drag(
    mut knobs: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &mut EmberKnob)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, rcp, mut k) in &mut knobs {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let v = (0.5 - n.y).clamp(0.0, 1.0);
        if (v - k.value).abs() < 0.001 {
            continue;
        }
        k.value = v;
        let (lx, ty) = knob_offset(v);
        if let Ok(mut node) = nodes.get_mut(k.indicator) {
            node.left = Val::Px(lx);
            node.top = Val::Px(ty);
        }
    }
}
