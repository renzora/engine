//! Snaps back to the spawn point when the respawn key is pressed. The home
//! position is captured on the first frame, and velocity is zeroed on respawn
//! so the body does not carry its momentum home.
//!
//! Setup: attach to the player; press R to respawn.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

const RESPAWN_KEY: KeyCode = KeyCode::KeyR;

#[derive(Component)]
struct Home {
    position: Vec3,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<Home>() {
        let Some(position) = ctx.get::<Transform>().map(|t| t.translation) else { return };
        ctx.insert(Home { position });
    }
    let pressed = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(RESPAWN_KEY));
    if !pressed {
        return;
    }
    let Some(home) = ctx.get::<Home>().map(|h| h.position) else { return };

    let me = ctx.entity();
    ScriptAction::new("set_velocity", me)
        .arg("x", 0.0)
        .arg("y", 0.0)
        .arg("z", 0.0)
        .send(ctx.world());
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = home;
    }
}

renzora::script!(update);
