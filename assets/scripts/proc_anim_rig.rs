//! A procedural locomotion blender: cross-fade idle, walk and run layers from
//! movement input instead of snapping between clips. Target weights come from
//! the input magnitude as a 1D blend space and are smoothed toward, which
//! avoids popping. E fires an upper-body action layer that fades in at once
//! and decays.
//!
//! Setup: attach to a rigged character with locomotion layers "0" (idle),
//! "1" (walk), "2" (run) and an action layer "3", a `speed` parameter and a
//! `wave` trigger. WASD moves the blend, Shift runs.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

/// Layer names for idle, walk and run, in that order.
const LOCO: [&str; 3] = ["0", "1", "2"];
const ACTION_LAYER: &str = "3";
/// How fast locomotion weights chase their targets.
const LOCO_RATE: f32 = 8.0;
/// How fast the action overlay decays.
const ACTION_RATE: f32 = 3.0;

#[derive(Component)]
struct Blend {
    weights: [f32; 3],
    action: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let me = ctx.entity();
    let (movement, shift, wave) = match ctx.get_resource::<ScriptInput>() {
        Some(i) => (
            i.get_movement_vector(),
            i.is_key_pressed(KeyCode::ShiftLeft),
            i.is_key_just_pressed(KeyCode::KeyE),
        ),
        None => (Vec2::ZERO, false, false),
    };
    let speed = movement.length().clamp(0.0, 1.0);
    let running = shift && speed > 0.1;
    let targets = [1.0 - speed, if running { 0.0 } else { speed }, if running { speed } else { 0.0 }];

    let Some(mut blend) = ctx.get_mut::<Blend>() else { return };
    for (w, target) in blend.weights.iter_mut().zip(targets) {
        *w = smooth(*w, target, LOCO_RATE, dt);
    }
    if wave {
        blend.action = 1.0;
    }
    blend.action = smooth(blend.action, 0.0, ACTION_RATE, dt);
    let (weights, action) = (blend.weights, blend.action);
    drop(blend);

    let world = ctx.world();
    for (layer, weight) in LOCO.iter().zip(weights) {
        set_layer_weight(world, me, layer, weight);
    }
    ScriptAction::new("set_anim_param", me).arg("name", "speed").arg("value", speed).send(world);
    if wave {
        ScriptAction::new("trigger_anim", me).arg("name", "wave").send(world);
    }
    set_layer_weight(world, me, ACTION_LAYER, action);
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    ctx.insert(Blend { weights: [1.0, 0.0, 0.0], action: 0.0 });
    let me = ctx.entity();
    let world = ctx.world();
    ScriptAction::new("play_animation", me)
        .arg("name", "idle")
        .arg("looping", true)
        .arg("speed", 1.0)
        .send(world);
    set_layer_weight(world, me, LOCO[0], 1.0);
}

/// Frame-rate independent approach toward `target`.
fn smooth(cur: f32, target: f32, rate: f32, dt: f32) -> f32 {
    cur + (target - cur) * (dt * rate).clamp(0.0, 1.0)
}

fn set_layer_weight(world: &mut World, me: Entity, layer: &str, weight: f32) {
    ScriptAction::new("set_layer_weight", me).arg("layer_name", layer).arg("weight", weight).send(world);
}

renzora::script!(update, hooks = hooks);
