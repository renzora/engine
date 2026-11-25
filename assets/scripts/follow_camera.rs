//! A chase camera that trails a target entity (a car, say) from behind and
//! above. It reads the target's position and yaw, eases toward a point
//! behind it with exponential smoothing (frame-rate independent), and aims
//! slightly above the target's origin.
//!
//! Setup: attach to the camera; the entity to follow is named "car"
//! (`TARGET`).

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook};

/// Name of the entity to follow.
const TARGET: &str = "car";
/// Trail distance behind the target.
const DISTANCE: f32 = 8.0;
/// Height above the target.
const HEIGHT: f32 = 3.5;
/// Aim point height above the target's origin.
const LOOK_HEIGHT: f32 = 1.0;
/// Position smoothing; higher is snappier.
const FOLLOW_SMOOTH: f32 = 6.0;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(target) = find_named(ctx.world(), TARGET) else { return };
    let Some((target_pos, target_rot)) =
        ctx.get_on::<Transform>(target).map(|t| (t.translation, t.rotation))
    else {
        return;
    };
    let (yaw, _, _) = target_rot.to_euler(EulerRot::YXZ);

    // "Behind" is the target's +Z, since Bevy models face -Z.
    let desired = target_pos + Vec3::new(yaw.sin() * DISTANCE, HEIGHT, yaw.cos() * DISTANCE);
    let t = 1.0 - (-FOLLOW_SMOOTH * dt).exp();
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = transform.translation.lerp(desired, t);
        transform.look_at(target_pos + Vec3::Y * LOOK_HEIGHT, Vec3::Y);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let me = ctx.entity();
        ScriptAction::new("lock_cursor", me).send(ctx.world());
    }
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update, hooks = hooks);
