//! Respawn on fall: when the character drops below `KILL_Y` it snaps back to
//! a respawn point, catching it after it walks off a ledge or falls through
//! the world. The point is where the character started, or a fixed point
//! (a checkpoint) when `USE_CUSTOM_SPAWN` is set.
//!
//! Setup: attach to the character.

use bevy::prelude::*;
use renzora::ScriptCtx;

const KILL_Y: f32 = -15.0;
const USE_CUSTOM_SPAWN: bool = false;
const CUSTOM_SPAWN: Vec3 = Vec3::new(0.0, 2.0, 0.0);

/// The start position, captured on the first frame.
#[derive(Component)]
struct Spawn(Vec3);

fn update(ctx: &mut ScriptCtx) {
    let Some(position) = ctx.get::<Transform>().map(|t| t.translation) else { return };
    let start = match ctx.get::<Spawn>() {
        Some(spawn) => spawn.0,
        None => {
            ctx.insert(Spawn(position));
            position
        }
    };
    if position.y >= KILL_Y {
        return;
    }
    let point = if USE_CUSTOM_SPAWN { CUSTOM_SPAWN } else { start };
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = point;
    }
}

renzora::script!(update);
