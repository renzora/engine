//! Additive upper-body layering. A base locomotion clip plays on layer 0 while
//! an aim overlay rides layer 1. The layer-1 weight is ramped in and out so
//! the overlay fades smoothly instead of snapping the upper body into place.
//!
//! Setup: a character with a layered animator; layer 1 holds the overlay clip
//! "aim_overlay". Hold the right mouse button (or the right trigger) to aim.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

/// How fast the overlay weight chases its target, per second.
const FADE_SPEED: f32 = 4.0;
const OVERLAY_LAYER: &str = "1";

#[derive(Component, Default)]
struct Overlay {
    weight: f32,
    target: f32,
    on: bool,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let mouse = ctx
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|b| b.pressed(MouseButton::Right));
    let trigger = ctx.get_resource::<ScriptInput>().is_some_and(|input| {
        input.first_gamepad().is_some_and(|pad| input.get_gamepad_trigger(pad, false) > 0.5)
    });
    let engage = mouse || trigger;

    let Some(mut overlay) = ctx.get_mut::<Overlay>() else { return };
    let engaged_now = engage != overlay.on && engage;
    if engage != overlay.on {
        overlay.on = engage;
        overlay.target = if engage { 1.0 } else { 0.0 };
    }
    let mut new_weight = None;
    if overlay.weight != overlay.target {
        let mut w = overlay.weight + (overlay.target - overlay.weight) * (dt * FADE_SPEED).clamp(0.0, 1.0);
        // Snap the tail so the lerp actually arrives instead of creeping.
        if (w - overlay.target).abs() < 0.01 {
            w = overlay.target;
        }
        overlay.weight = w;
        new_weight = Some(w);
    }
    drop(overlay);

    let me = ctx.entity();
    if engaged_now {
        ScriptAction::new("crossfade_animation", me)
            .arg("name", "aim_overlay")
            .arg("duration", 0.1)
            .arg("looping", true)
            .send(ctx.world());
    }
    if let Some(w) = new_weight {
        set_layer_weight(ctx, w);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        ctx.insert(Overlay::default());
        let me = ctx.entity();
        ScriptAction::new("play_animation", me)
            .arg("name", "idle")
            .arg("looping", true)
            .arg("speed", 1.0)
            .send(ctx.world());
        // The overlay starts silent.
        set_layer_weight(ctx, 0.0);
    }
}

fn set_layer_weight(ctx: &mut ScriptCtx, weight: f32) {
    let me = ctx.entity();
    ScriptAction::new("set_layer_weight", me)
        .arg("layer_name", OVERLAY_LAYER)
        .arg("weight", weight)
        .send(ctx.world());
}

renzora::script!(update, hooks = hooks);
