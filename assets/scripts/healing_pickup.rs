//! A floating health pickup. It bobs and spins for readability, and when the
//! player comes within the pickup radius it heals them, plays a chime, then
//! despawns itself so it cannot heal twice.
//!
//! The engine has no `Health` component (the Lua original's reads of
//! `Health.current` always came back empty), so health belongs to the
//! player's own script. The pickup emits a "heal" game event carrying
//! `amount` and `target` (the player's entity bits); the player's script
//! handles it in `ScriptHook::Event` and clamps to its own maximum.
//!
//! Setup: place in the world; one entity is named "player".

use std::collections::HashMap;

use bevy::prelude::*;
use renzora::script_bridge::{GameEvent, GameEventQueue, ScriptAction, ScriptActionValue};
use renzora::{ScriptCtx, ScriptHook};

const HEAL: f32 = 25.0;
const RADIUS: f32 = 1.5;
const BOB_HEIGHT: f32 = 0.25;
/// Spin in degrees per second.
const SPIN_SPEED: f32 = 90.0;
const TARGET: &str = "player";

#[derive(Component)]
struct Pickup {
    base_y: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let elapsed = ctx.elapsed();
    let Some(base_y) = ctx.get::<Pickup>().map(|p| p.base_y) else { return };
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation.y = base_y + (elapsed * 3.0).sin() * BOB_HEIGHT;
    transform.rotate_y((SPIN_SPEED * dt).to_radians());
    let here = transform.translation;
    drop(transform);

    let Some(player) = find_named(ctx.world(), TARGET) else { return };
    let Some(there) = ctx.get_on::<Transform>(player).map(|t| t.translation) else { return };
    if (there - here).xz().length_squared() > RADIUS * RADIUS {
        return;
    }

    let me = ctx.entity();
    let world = ctx.world();
    let args = HashMap::from([
        ("amount".to_string(), ScriptActionValue::Float(HEAL)),
        ("target".to_string(), ScriptActionValue::Int(player.to_bits() as i64)),
    ]);
    world
        .get_resource_or_insert_with(GameEventQueue::default)
        .pending
        .push(GameEvent { name: "heal".into(), args, from: Some(me) });
    ScriptAction::new("play_sound", me)
        .arg("path", "pickup_heal")
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(world);
    world.despawn(me);
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let base_y = ctx.get::<Transform>().map_or(0.0, |t| t.translation.y);
        ctx.insert(Pickup { base_y });
    }
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update, hooks = hooks);
