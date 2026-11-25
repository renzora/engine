//! Mouse look that accumulates pitch and yaw from raw mouse deltas. It keeps
//! its own yaw and pitch and clamps pitch so the view cannot flip over the
//! poles, then rebuilds the rotation from them each frame.
//!
//! Setup: attach to a camera or head entity. The cursor is locked when the
//! script starts.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

/// Degrees per mouse unit.
const SENSITIVITY: f32 = 0.15;
/// Pitch limit in degrees, just short of straight up or down.
const PITCH_LIMIT: f32 = 89.0;

#[derive(Component)]
struct Look {
    yaw: f32,
    pitch: f32,
}

fn update(ctx: &mut ScriptCtx) {
    ensure_state(ctx);
    let delta = ctx
        .get_resource::<ScriptInput>()
        .map(|input| input.mouse_delta)
        .unwrap_or(Vec2::ZERO);
    let Some(mut look) = ctx.get_mut::<Look>() else { return };
    look.yaw -= delta.x * SENSITIVITY;
    look.pitch = (look.pitch - delta.y * SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    let (yaw, pitch) = (look.yaw, look.pitch);
    drop(look);

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw.to_radians(), pitch.to_radians(), 0.0);
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        ensure_state(ctx);
        let me = ctx.entity();
        ScriptAction::new("lock_cursor", me).send(ctx.world());
    }
}

/// Seeds the accumulators from the authored rotation, so the view does not
/// snap to zero on the first frame.
fn ensure_state(ctx: &mut ScriptCtx) {
    if ctx.has::<Look>() {
        return;
    }
    let Some(rotation) = ctx.get::<Transform>().map(|t| t.rotation) else { return };
    let (y, x, _) = rotation.to_euler(EulerRot::YXZ);
    ctx.insert(Look { yaw: y.to_degrees(), pitch: x.to_degrees() });
}

renzora::script!(update, hooks = hooks);
