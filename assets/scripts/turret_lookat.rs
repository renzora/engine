//! A turret that tracks the entity named "player". It turns to face the
//! player while within range, and fires bolts named "turret_bolt" on a
//! cooldown so it does not spew one every frame while aimed. Out of range it
//! holds its last facing.
//!
//! Setup: place near the player; exactly one entity is named "player".

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;

/// Planar tracking range.
const RANGE: f32 = 20.0;
/// Seconds between shots.
const FIRE_COOLDOWN: f32 = 0.8;
const PREFIX: &str = "turret_bolt";
const TARGET: &str = "player";

#[derive(Component, Default)]
struct Turret {
    cooldown: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Turret>() {
        ctx.insert(Turret::default());
    }
    let Some(player) = find_named(ctx.world(), TARGET) else { return };
    let Some(target) = ctx.get_on::<Transform>(player).map(|t| t.translation) else { return };

    let Some(mut turret) = ctx.get_mut::<Turret>() else { return };
    turret.cooldown = (turret.cooldown - dt).max(0.0);
    let ready = turret.cooldown <= 0.0;
    drop(turret);

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    let offset = (target - transform.translation).xz();
    if offset.length_squared() > RANGE * RANGE {
        return;
    }
    transform.look_at(target, Vec3::Y);
    let dir = offset.normalize_or_zero();
    let muzzle = transform.translation + Vec3::new(dir.x, 0.5, dir.y);
    drop(transform);

    if !ready {
        return;
    }
    if let Some(mut turret) = ctx.get_mut::<Turret>() {
        turret.cooldown = FIRE_COOLDOWN;
    }
    let me = ctx.entity();
    let world = ctx.world();
    world.spawn((
        Name::new(PREFIX),
        Transform::from_translation(muzzle),
        Visibility::default(),
        MeshPrimitive("sphere".into()),
        MeshColor(Color::srgb(1.0, 0.3, 0.1)),
    ));
    ScriptAction::new("play_sound", me)
        .arg("path", "turret_fire")
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(world);
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
