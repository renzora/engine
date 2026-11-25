//! Spawns a ring of primitives in one burst, for shockwave debris or a
//! spawn-point telegraph. Every chunk carries the same name prefix, so one
//! prefix sweep clears the whole ring later without tracking handles.
//!
//! Setup: attach to any entity with a Transform. B fires a burst around it;
//! C clears every chunk spawned so far.

use std::f32::consts::TAU;

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

const COUNT: usize = 12;
const RADIUS: f32 = 3.0;
const PREFIX: &str = "ring_chunk";

fn update(ctx: &mut ScriptCtx) {
    let (burst, clear) = match ctx.get_resource::<ScriptInput>() {
        Some(input) => (
            input.is_key_just_pressed(KeyCode::KeyB),
            input.is_key_just_pressed(KeyCode::KeyC),
        ),
        None => return,
    };
    if burst {
        spawn_ring(ctx);
    }
    if clear {
        despawn_by_prefix(ctx.world(), PREFIX);
    }
}

fn spawn_ring(ctx: &mut ScriptCtx) {
    let Some(origin) = ctx.get::<Transform>().map(|t| t.translation) else { return };
    let me = ctx.entity();
    let world = ctx.world();
    for i in 0..COUNT {
        let f = i as f32 / COUNT as f32;
        let a = f * TAU;
        let pos = origin + Vec3::new(a.cos() * RADIUS, 0.5, a.sin() * RADIUS);
        // The colour cycles around the ring so the burst reads as a wheel.
        world.spawn((
            Name::new(PREFIX),
            Transform::from_translation(pos),
            Visibility::default(),
            MeshPrimitive("sphere".into()),
            MeshColor(Color::srgb(f, 1.0 - f, 0.5)),
        ));
    }
    ScriptAction::new("play_sound", me)
        .arg("path", "spawn")
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
