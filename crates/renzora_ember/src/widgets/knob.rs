//! Knob — a rotary control rendered as an arc dial (shared with the gauge);
//! drag vertically to change its value.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::SystemCursorIcon;

use super::drag_value::{WheelOverDragValue, GESTURE_GAP};
use super::gauge::ArcData;
use crate::reactive::Bound;
use crate::UiInteraction;

#[derive(Component)]
pub(crate) struct EmberKnob;

/// Opt a knob into mouse-wheel adjustment while hovered: `step` per notch, and
/// `fine_step` per notch with Ctrl held. Both are in the knob's own 0..1 space.
///
/// Opt-in rather than on every knob because a plain wheel over a control that
/// sits inside a scroll view takes the wheel away from that view, which is why
/// [`super::drag_value`] fields want Shift. A knob that is the obvious target
/// under the cursor (the mixer's pan) is worth that trade; a knob in a long
/// inspector might not be.
///
/// Crossing the knob's pivot snaps onto it (a pan knob lands on exactly centre
/// instead of stepping from L2 straight to R3), and the wheel is swallowed from
/// the scroll view behind for as long as the knob owns the gesture.
#[derive(Component, Clone, Copy, Debug)]
pub struct KnobWheel {
    pub step: f32,
    pub fine_step: f32,
}

/// Which knob (if any) owns the in-progress wheel gesture. Decided when a
/// gesture starts (after a [`GESTURE_GAP`] of quiet) and sticky for its length,
/// the same rule the value fields use: a panel scroll that drifts across a knob
/// keeps scrolling the panel instead of snagging on it and changing a value.
#[derive(Default)]
pub(crate) struct KnobWheelGesture {
    last_t: f32,
    owner: Option<Entity>,
}

/// Wheel over a [`KnobWheel`] knob nudges its `Bound<f32>`, so a `bind_2way`
/// writes it through exactly the path a drag does. Runs after the value-field
/// pass (which overwrites [`WheelOverDragValue`] every frame) and before the
/// scroll areas read it.
pub(crate) fn knob_wheel(
    mut wheel: MessageReader<MouseWheel>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut capture: ResMut<WheelOverDragValue>,
    mut gesture: Local<KnobWheelGesture>,
    mut knobs: Query<(Entity, &UiInteraction, &KnobWheel, &ArcData, &mut Bound<f32>), With<EmberKnob>>,
) {
    // Notches, not raw deltas: a touchpad / precision wheel reports pixels, and
    // taking those as notches would fling the knob end to end in one flick.
    let mut notches = 0.0;
    for ev in wheel.read() {
        notches += match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 40.0,
        };
    }
    if notches == 0.0 {
        return;
    }
    let hovered = knobs
        .iter()
        .find(|(_, i, ..)| matches!(i, UiInteraction::Hovered | UiInteraction::Pressed))
        .map(|(e, ..)| e);
    let now = time.elapsed_secs();
    if now - gesture.last_t > GESTURE_GAP {
        gesture.owner = hovered;
    }
    gesture.last_t = now;
    let Some(owner) = gesture.owner.filter(|o| Some(*o) == hovered) else {
        return;
    };
    let Ok((_, _, wheel_cfg, arc, mut bound)) = knobs.get_mut(owner) else {
        return;
    };
    capture.0 = true;

    let fine = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let step = if fine { wheel_cfg.fine_step } else { wheel_cfg.step };
    let old = bound.0;
    let mut v = (old + notches * step).clamp(0.0, 1.0);
    // Snap onto the pivot when the step would jump over it. Only an interior
    // pivot counts: a fill-from-the-left knob's pivot is its end stop.
    let p = arc.pivot;
    if p > 0.0 && p < 1.0 && (old - p) * (v - p) < 0.0 {
        v = p;
    }
    if (v - old).abs() > f32::EPSILON {
        bound.0 = v;
    }
}

/// A rotary knob (drag vertically to change `value` 0..1). The filled arc shows
/// the value. Carries `Bound<f32>` so it can be two-way bound with `bind_2way`.
pub fn knob(commands: &mut Commands, value: f32) -> Entity {
    knob_pivoted(commands, value, 0.0)
}

/// A knob whose fill spans out from `pivot` rather than from the low end.
///
/// `pivot: 0.5` is the one worth knowing about: it makes a **pan** knob. Pan has
/// a centre and two directions away from it, so a fill that always starts at the
/// left end draws a half-full dial for dead centre — which reads as "panned
/// left", and was reported as exactly that.
pub fn knob_pivoted(commands: &mut Commands, value: f32, pivot: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    commands
        .spawn((
            Node {
                width: Val::Px(46.0),
                height: Val::Px(46.0),
                ..default()
            },
            ArcData { value: v, pivot: pivot.clamp(0.0, 1.0) },
            EmberKnob,
            Bound::<f32>(v),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::NsResize),
            Name::new("knob"),
        ))
        .id()
}

/// User drag → write the model (`Bound<f32>`). The arc follows via [`knob_apply`].
pub(crate) fn knob_drag(
    mut knobs: Query<(&UiInteraction, &RelativeCursorPosition, &mut Bound<f32>), With<EmberKnob>>,
) {
    for (interaction, rcp, mut b) in &mut knobs {
        if *interaction != UiInteraction::Pressed {
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
