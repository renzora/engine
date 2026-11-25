//! First-person mouse look for a camera. Pitch and yaw are accumulated in the
//! script's own state rather than read back from the rotation each frame, so
//! the view never drifts. Escape frees the cursor; a left click locks it
//! again.
//!
//! Setup: attach to the camera entity. Pair the character with a movement
//! script.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

/// Degrees turned per pixel of mouse movement.
const SENSITIVITY: f32 = 0.15;
const INVERT_Y: bool = false;
/// Maximum look up or down, in degrees.
const PITCH_LIMIT: f32 = 89.0;

/// Accumulated look angles, in degrees.
#[derive(Component)]
struct Look {
    yaw: f32,
    pitch: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let (delta, escape) = match ctx.get_resource::<ScriptInput>() {
        Some(input) => (input.mouse_delta, input.is_key_just_pressed(KeyCode::Escape)),
        None => (Vec2::ZERO, false),
    };
    let clicked = ctx
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|b| b.just_pressed(MouseButton::Left));

    let Some(mut look) = ctx.get_mut::<Look>() else { return };
    look.yaw -= delta.x * SENSITIVITY;
    let dy = if INVERT_Y { delta.y } else { -delta.y };
    look.pitch = (look.pitch + dy * SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    let rotation =
        Quat::from_euler(EulerRot::YXZ, look.yaw.to_radians(), look.pitch.to_radians(), 0.0);
    drop(look);
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.rotation = rotation;
    }

    let me = ctx.entity();
    if escape {
        ScriptAction::new("unlock_cursor", me).send(ctx.world());
    }
    if clicked {
        ScriptAction::new("lock_cursor", me).send(ctx.world());
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let (yaw, pitch, _) = ctx
            .get::<Transform>()
            .map_or((0.0, 0.0, 0.0), |t| t.rotation.to_euler(EulerRot::YXZ));
        ctx.insert(Look { yaw: yaw.to_degrees(), pitch: pitch.to_degrees() });
        let me = ctx.entity();
        ScriptAction::new("lock_cursor", me).send(ctx.world());
    }
}

renzora::script!(update, hooks = hooks);
