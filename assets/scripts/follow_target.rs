//! Follows another entity found by name, so one entity can track another with
//! no hard link between them. It stops short by a keep distance so it never
//! overlaps the target.
//!
//! Setup: attach to the follower. The target is the entity named "player";
//! change `TARGET_NAME` to follow something else.

use bevy::prelude::*;
use renzora::ScriptCtx;

const TARGET_NAME: &str = "player";
/// Units per second.
const SPEED: f32 = 4.0;
/// Stop this far away from the target.
const KEEP_DISTANCE: f32 = 2.0;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(target) = find_named(ctx.world(), TARGET_NAME) else { return };
    let Some(target_pos) = ctx.get_on::<Transform>(target).map(|t| t.translation) else { return };
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };

    let offset = Vec2::new(target_pos.x - transform.translation.x, target_pos.z - transform.translation.z);
    let dist = offset.length();
    if dist <= KEEP_DISTANCE || dist == 0.0 {
        return;
    }
    let dir = offset / dist;
    transform.translation += Vec3::new(dir.x, 0.0, dir.y) * SPEED * dt;
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
