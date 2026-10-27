//! Toggle switch: a bindable pill on/off control.
//!
//! Distinct from [`super::toggle`] (internal state): this one carries
//! `Bound<bool>` so `bind_2way` drives it both ways. "On" is the theme accent,
//! not a fixed green, so a switch reads as the same "active" color as the
//! selection and primary buttons beside it; green stays reserved for status.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::interaction::PASS_THROUGH;
use crate::reactive::Bound;
use crate::theme::{accent, rgb};
use crate::UiInteraction;

/// Fixed rather than a palette surface: the off track must stay visible against
/// every row, header and card it sits on, and no surface token is reliably
/// lighter than all of them.
const OFF_BG: (u8, u8, u8) = (58, 64, 80);
const KNOB: (u8, u8, u8) = (245, 245, 250);
const W: f32 = 28.0;
const H: f32 = 14.0;
const KNOB_SZ: f32 = 10.0;

#[derive(Component)]
pub(crate) struct EmberSwitch {
    knob: Entity,
}

fn knob_x(on: bool) -> f32 {
    let inset = (H - KNOB_SZ) / 2.0;
    if on {
        W - KNOB_SZ - inset
    } else {
        inset
    }
}

/// A pill on/off switch (slate off, accent on). Click to flip; `Bound<bool>` holds
/// the state so `bind_2way` can drive it.
pub fn toggle_switch(commands: &mut Commands, on: bool) -> Entity {
    let knob = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(KNOB_SZ),
                height: Val::Px(KNOB_SZ),
                top: Val::Px((H - KNOB_SZ) / 2.0),
                left: Val::Px(knob_x(on)),
                border_radius: BorderRadius::all(Val::Px(KNOB_SZ / 2.0)),
                ..default()
            },
            BackgroundColor(rgb(KNOB)),
            PASS_THROUGH,
            Name::new("switch-knob"),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                width: Val::Px(W),
                height: Val::Px(H),
                position_type: PositionType::Relative,
                border_radius: BorderRadius::all(Val::Px(H / 2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(if on { accent() } else { OFF_BG })),
            UiInteraction::default(),
            EmberSwitch { knob },
            Bound::<bool>(on),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("toggle-switch"),
        ))
        .id();
    commands.entity(track).add_child(knob);
    track
}

/// Click → flip the model (`Bound<bool>`); visuals follow via [`switch_apply`].
pub(crate) fn switch_interact(
    mut q: Query<(&UiInteraction, &mut Bound<bool>), (With<EmberSwitch>, Changed<UiInteraction>)>,
) {
    for (interaction, mut b) in &mut q {
        if *interaction == UiInteraction::Pressed {
            b.0 = !b.0;
        }
    }
}

/// Model (`Bound<bool>`) → track color + knob position (click or external push).
pub(crate) fn switch_apply(
    mut q: Query<(&EmberSwitch, &Bound<bool>, &mut BackgroundColor), Changed<Bound<bool>>>,
    mut nodes: Query<&mut Node>,
) {
    for (sw, b, mut bg) in &mut q {
        bg.0 = rgb(if b.0 { accent() } else { OFF_BG });
        if let Ok(mut n) = nodes.get_mut(sw.knob) {
            n.left = Val::Px(knob_x(b.0));
        }
    }
}
