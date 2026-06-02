//! Knob — a rotary control rendered as an arc dial (shared with the gauge);
//! drag vertically to change its value.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::SystemCursorIcon;

use super::gauge::ArcData;
use crate::reactive::Bound;

#[derive(Component)]
pub(crate) struct EmberKnob;

/// A rotary knob (drag vertically to change `value` 0..1). The filled arc shows
/// the value. Carries `Bound<f32>` so it can be two-way bound with `bind_2way`.
pub fn knob(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    commands
        .spawn((
            Node {
                width: Val::Px(46.0),
                height: Val::Px(46.0),
                ..default()
            },
            ArcData { value: v },
            EmberKnob,
            Bound::<f32>(v),
            Interaction::default(),
            RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::NsResize),
            Name::new("knob"),
        ))
        .id()
}

/// User drag → write the model (`Bound<f32>`). The arc follows via [`knob_apply`].
pub(crate) fn knob_drag(
    mut knobs: Query<(&Interaction, &RelativeCursorPosition, &mut Bound<f32>), With<EmberKnob>>,
) {
    for (interaction, rcp, mut b) in &mut knobs {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let v = (0.5 - n.y).clamp(0.0, 1.0);
        if (v - b.0).abs() >= 0.001 {
            b.0 = v;
        }
    }
}

/// Model (`Bound<f32>`) → arc value (user drag or a `bind_2way` state push).
pub(crate) fn knob_apply(
    mut knobs: Query<(&mut ArcData, &Bound<f32>), (With<EmberKnob>, Changed<Bound<f32>>)>,
) {
    for (mut arc, b) in &mut knobs {
        arc.value = b.0.clamp(0.0, 1.0);
    }
}
