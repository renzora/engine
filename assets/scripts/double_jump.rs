//! A second mid-air jump. Jumps are counted until the body lands, and the
//! count is capped, which gives the classic double jump. Each jump sets the
//! vertical velocity outright, so the second one is always full height.
//!
//! Landing is read from the body's physics contacts: any contact that pushes
//! the body upward counts as ground. The Lua version used `is_colliding`,
//! which nothing filled in, so its jumps never refilled.
//!
//! Setup: attach to a dynamic rigid body with a collider; press Space to jump.

use bevy::prelude::*;
use renzora::physics::PhysicsContacts;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

/// Upward speed each jump sets.
const JUMP_FORCE: f32 = 6.0;
/// Total jumps before landing again.
const MAX_JUMPS: u32 = 2;
/// A contact normal this close to straight up counts as floor, not wall.
const FLOOR_NORMAL_Y: f32 = 0.5;

#[derive(Component)]
struct Jumps {
    used: u32,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<Jumps>() {
        ctx.insert(Jumps { used: 0 });
    }
    let on_ground = ctx
        .get::<PhysicsContacts>()
        .is_some_and(|contacts| contacts.0.iter().any(|c| c.normal.y > FLOOR_NORMAL_Y));
    let pressed = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(KeyCode::Space));

    let Some(mut jumps) = ctx.get_mut::<Jumps>() else { return };
    if on_ground {
        jumps.used = 0;
    }
    if !pressed || jumps.used >= MAX_JUMPS {
        return;
    }
    jumps.used += 1;
    drop(jumps);

    let me = ctx.entity();
    ScriptAction::new("set_velocity", me)
        .arg("x", 0.0)
        .arg("y", JUMP_FORCE)
        .arg("z", 0.0)
        .send(ctx.world());
}

renzora::script!(update);
