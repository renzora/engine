//! A forgiving platformer jump. Coyote time lets a jump count a moment after
//! leaving a ledge, and a jump buffer lets a press made just before touchdown
//! still count. Both hide input latency and feel far better than a strict
//! grounded check.
//!
//! The Lua version moved W toward +Z; W now moves forward (-Z), as in the other
//! movement scripts.
//!
//! Setup: attach to a kinematic character body; Space jumps.

use bevy::prelude::*;
use renzora::core::reflection::get_reflected_field;
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::{ScriptCtx, ScriptInput};

const MOVE_SPEED: f32 = 6.0;
/// Upward speed a jump sets.
const JUMP_FORCE: f32 = 9.0;
const GRAVITY: f32 = -22.0;
/// Seconds of grace after leaving the ground.
const COYOTE: f32 = 0.12;
/// Seconds a jump press stays queued.
const BUFFER: f32 = 0.12;
/// Downward speed kept while grounded, so the body stays seated.
const GROUND_STICK: f32 = -1.0;

#[derive(Component)]
struct Platformer {
    vertical_velocity: f32,
    coyote_left: f32,
    buffer_left: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let me = ctx.entity();
    if !ctx.has::<Platformer>() {
        ctx.insert(Platformer { vertical_velocity: 0.0, coyote_left: 0.0, buffer_left: 0.0 });
    }
    let (axis, pressed) = match ctx.get_resource::<ScriptInput>() {
        Some(input) => (input.get_movement_vector(), input.is_key_just_pressed(KeyCode::Space)),
        None => (Vec2::ZERO, false),
    };
    let grounded = matches!(
        get_reflected_field(ctx.world(), me, "PhysicsReadState", "grounded"),
        Some(PropertyValue::Bool(true))
    );

    let Some(mut state) = ctx.get_mut::<Platformer>() else { return };
    state.coyote_left = if grounded { COYOTE } else { state.coyote_left - dt };
    state.buffer_left = if pressed { BUFFER } else { state.buffer_left - dt };

    // A buffered press meeting available coyote time is a jump.
    let jumped = state.buffer_left > 0.0 && state.coyote_left > 0.0;
    if jumped {
        state.vertical_velocity = JUMP_FORCE;
        state.buffer_left = 0.0;
        state.coyote_left = 0.0;
    }
    if grounded && state.vertical_velocity <= 0.0 {
        state.vertical_velocity = GROUND_STICK;
    } else {
        state.vertical_velocity += GRAVITY * dt;
    }
    let vy = state.vertical_velocity;
    drop(state);

    let world = ctx.world();
    if jumped {
        ScriptAction::new("play_sound", me)
            .arg("path", "jump")
            .arg("volume", 1.0)
            .arg("looping", false)
            .arg("bus", "Sfx")
            .send(world);
        ScriptAction::new("trigger_anim", me).arg("name", "Jump").send(world);
    }
    ScriptAction::new("kinematic_slide", me)
        .arg("x", axis.x * MOVE_SPEED * dt)
        .arg("y", vy * dt)
        .arg("z", -axis.y * MOVE_SPEED * dt)
        .send(world);
}

renzora::script!(update);
