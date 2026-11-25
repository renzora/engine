//! Smooth follow: eases toward a named target each frame instead of snapping,
//! which gives the springy "camera lag" feel. A higher smoothing value reacts
//! faster, and scaling it by delta keeps the feel frame-rate independent.
//!
//! Setup: attach to a camera rig or companion. The target is the entity named
//! "player".

use bevy::prelude::*;
use renzora::ScriptCtx;

const TARGET_NAME: &str = "player";
/// Higher is snappier.
const SMOOTHING: f32 = 5.0;
/// Height kept above the target.
const OFFSET_Y: f32 = 3.0;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(target) = find_named(ctx.world(), TARGET_NAME) else { return };
    let Some(target_pos) = ctx.get_on::<Transform>(target).map(|t| t.translation) else { return };
    let t = (SMOOTHING * dt).clamp(0.0, 1.0);
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    let goal = target_pos + Vec3::Y * OFFSET_Y;
    transform.translation = transform.translation.lerp(goal, t);
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
