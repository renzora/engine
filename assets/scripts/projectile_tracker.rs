//! A fire-and-forget shooter. Each shot is a spawned primitive sharing the
//! "bolt" prefix, with no per-bolt handle kept. Instead a repeating sweep
//! clears the whole prefix every few seconds, so old bolts cannot leak forever.
//!
//! Setup: attach to a muzzle entity; hold F (or the gamepad right trigger) to
//! fire.

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

/// Seconds between shots while firing is held.
const FIRE_COOLDOWN: f32 = 0.18;
/// How often old bolts are culled, in seconds.
const SWEEP_INTERVAL: f32 = 3.0;
const PREFIX: &str = "bolt";

#[derive(Component)]
struct Shooter {
    cooldown: f32,
    sweep: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Shooter>() {
        ctx.insert(Shooter { cooldown: 0.0, sweep: SWEEP_INTERVAL });
    }
    let firing = ctx.get_resource::<ScriptInput>().is_some_and(|input| {
        input.is_key_pressed(KeyCode::KeyF)
            || input.first_gamepad().is_some_and(|pad| input.get_gamepad_trigger(pad, false) > 0.5)
    });

    let Some(mut shooter) = ctx.get_mut::<Shooter>() else { return };
    shooter.cooldown = (shooter.cooldown - dt).max(0.0);
    let fire = firing && shooter.cooldown <= 0.0;
    if fire {
        shooter.cooldown = FIRE_COOLDOWN;
    }
    shooter.sweep -= dt;
    let sweep = shooter.sweep <= 0.0;
    if sweep {
        shooter.sweep += SWEEP_INTERVAL;
    }
    drop(shooter);

    if fire {
        shoot(ctx);
    }
    if sweep {
        despawn_by_prefix(ctx.world(), PREFIX);
    }
}

fn shoot(ctx: &mut ScriptCtx) {
    let Some(transform) = ctx.get::<Transform>().map(|t| *t) else { return };
    // Spawn slightly in front of the muzzle along its yaw.
    let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
    let pos = transform.translation + Vec3::new(yaw.sin(), 0.0, yaw.cos()) * 0.8;
    let me = ctx.entity();
    let world = ctx.world();
    world.spawn((
        Name::new(PREFIX),
        Transform::from_translation(pos),
        Visibility::default(),
        MeshPrimitive("sphere".into()),
        MeshColor(Color::srgb(1.0, 0.8, 0.2)),
    ));
    ScriptAction::new("play_sound", me)
        .arg("path", "shoot")
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(world);
}

fn despawn_by_prefix(world: &mut World, prefix: &str) {
    let mut names = world.query::<(Entity, &Name)>();
    let doomed: Vec<Entity> = names
        .iter(world)
        .filter(|(_, n)| n.as_str().starts_with(prefix))
        .map(|(e, _)| e)
        .collect();
    for e in doomed {
        world.despawn(e);
    }
}

renzora::script!(update);
