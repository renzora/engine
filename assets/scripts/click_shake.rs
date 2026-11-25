//! Triggers a screen shake on a left mouse click. The shake is a one-shot
//! effect, so it fires only on the just-pressed edge rather than every frame
//! the button is held.
//!
//! Setup: attach to any entity.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;

/// Shake strength.
const INTENSITY: f32 = 0.4;
/// Seconds.
const DURATION: f32 = 0.25;

fn update(ctx: &mut ScriptCtx) {
    let clicked = ctx
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|buttons| buttons.just_pressed(MouseButton::Left));
    if !clicked {
        return;
    }
    let me = ctx.entity();
    ScriptAction::new("screen_shake", me)
        .arg("intensity", INTENSITY)
        .arg("duration", DURATION)
        .send(ctx.world());
}

renzora::script!(update);
