//! Slider — a draggable 0..1 value track.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::reactive::Bound;
use crate::theme::*;

/// Fill/thumb refs and the value range; the value itself lives in `Bound<f32>`
/// (so `bind_2way` can drive it).
///
/// The range lives HERE, not in whoever binds the slider. `Bound<f32>` then holds
/// the real value in the caller's units rather than a 0..1 fraction, so a binding
/// reads and writes a field directly with no mapping of its own — and a widget
/// swap can't silently change what a bound field means.
#[derive(Component)]
pub(crate) struct EmberSlider {
    fill: Entity,
    thumb: Entity,
    min: f32,
    max: f32,
}

impl EmberSlider {
    /// Value → 0..1 track fraction. A zero-width range would divide by zero;
    /// pinning it to 0 keeps a mis-specified slider harmless instead of NaN,
    /// which would propagate into the layout and blank the panel.
    fn fraction(&self, value: f32) -> f32 {
        let span = self.max - self.min;
        if span.abs() < f32::EPSILON {
            return 0.0;
        }
        ((value - self.min) / span).clamp(0.0, 1.0)
    }

    /// 0..1 track fraction → value.
    fn value(&self, fraction: f32) -> f32 {
        self.min + (self.max - self.min) * fraction.clamp(0.0, 1.0)
    }
}

/// A draggable slider with `value` in 0..1. Click/drag anywhere on it to set
/// the value.
pub fn slider(commands: &mut Commands, value: f32) -> Entity {
    slider_ranged(commands, value, 0.0, 1.0)
}

/// A draggable slider over an arbitrary range.
///
/// `value` is in `min..=max`, not 0..1 — which is what lets it drive a field like
/// a radius of 3.0 or a speed of 40 without the caller pre-normalising and
/// un-normalising on the way back. Inverted ranges (`max < min`) work, so a
/// slider can run right-to-left.
pub fn slider_ranged(commands: &mut Commands, value: f32, min: f32, max: f32) -> Entity {
    // The visual fraction, computed before the component exists to place the
    // fill/thumb on the first frame rather than waiting for `slider_apply`.
    let span = max - min;
    let v = if span.abs() < f32::EPSILON {
        0.0
    } else {
        ((value - min) / span).clamp(0.0, 1.0)
    };
    // 18px-tall hit area so it's easy to grab; the visual track is 6px.
    let row = commands
        .spawn((
            Node {
                width: Val::Px(160.0),
                height: Val::Px(18.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("slider"),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("slider-track"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(v * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            Name::new("slider-fill"),
        ))
        .id();
    let thumb = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(v * 100.0),
                margin: UiRect::left(Val::Px(-7.0)),
                width: Val::Px(14.0),
                height: Val::Px(14.0),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(on_accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("slider-thumb"),
        ))
        .id();
    commands.entity(track).add_child(fill);
    commands.entity(row).add_children(&[track, thumb]);
    // `Bound` carries the caller's value, not the fraction `v` used for layout.
    commands
        .entity(row)
        .insert((EmberSlider { fill, thumb, min, max }, Bound::<f32>(value)));
    row
}

/// User drag → write the model (`Bound<f32>`); visuals follow via [`slider_apply`].
pub(crate) fn slider_drag(
    mut sliders: Query<(
        &EmberSlider,
        &Interaction,
        &bevy::ui::RelativeCursorPosition,
        &mut Bound<f32>,
    )>,
) {
    for (slider, interaction, rcp, mut b) in &mut sliders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        // `normalized` is centered (-0.5..0.5); shift to 0..1, then out to the
        // slider's own range.
        let v = slider.value(n.x + 0.5);
        // Deadband scaled to the range, not a flat 0.001 — on a 0..1000 slider a
        // fixed epsilon makes every mouse jitter a write, and on a 0..0.01 one it
        // makes the slider unusable.
        let epsilon = ((slider.max - slider.min).abs() * 0.001).max(f32::EPSILON);
        if (v - b.0).abs() >= epsilon {
            b.0 = v;
        }
    }
}

/// Model (`Bound<f32>`) → fill/thumb (user drag or a `bind_2way` state push).
pub(crate) fn slider_apply(
    sliders: Query<(&EmberSlider, &Bound<f32>), Changed<Bound<f32>>>,
    mut nodes: Query<&mut Node>,
) {
    for (s, b) in &sliders {
        let v = s.fraction(b.0);
        if let Ok(mut fnode) = nodes.get_mut(s.fill) {
            fnode.width = Val::Percent(v * 100.0);
        }
        if let Ok(mut tnode) = nodes.get_mut(s.thumb) {
            tnode.left = Val::Percent(v * 100.0);
        }
    }
}
