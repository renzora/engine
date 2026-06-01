//! Knob — a rotary control rendered as an arc dial (shared with the gauge);
//! drag vertically to change its value.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::SystemCursorIcon;

use super::gauge::ArcData;

#[derive(Component)]
pub(crate) struct EmberKnob;

/// A rotary knob (drag vertically to change `value` 0..1). The filled arc shows
/// the value.
pub fn knob(commands: &mut Commands, value: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(46.0),
                height: Val::Px(46.0),
                ..default()
            },
            ArcData {
                value: value.clamp(0.0, 1.0),
            },
            EmberKnob,
            Interaction::default(),
            RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::NsResize),
            Name::new("knob"),
        ))
        .id()
}

pub(crate) fn knob_drag(
    mut knobs: Query<(&Interaction, &RelativeCursorPosition, &mut ArcData), With<EmberKnob>>,
) {
    for (interaction, rcp, mut data) in &mut knobs {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let v = (0.5 - n.y).clamp(0.0, 1.0);
        if (v - data.value).abs() < 0.001 {
            continue;
        }
        data.value = v;
    }
}
