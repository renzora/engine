//! Shows off a track (or any entity) from a series of fixed overhead vantage
//! points. Each shot is an elevated position at its own angle, distance and
//! height around the target; the camera pans slowly during a shot, holds for
//! `HOLD_TIME`, then cuts to the next and loops. N jumps to the next shot.
//!
//! The Lua version animated a "Screen Transition" effect (crossfade, wipe,
//! slide, iris) at each cut. That effect no longer exists in the engine, so
//! every change of shot is a hard cut.
//!
//! Setup: attach to the camera. The entity to frame is named "track".

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

const TARGET: &str = "track";
/// Seconds each shot is held before cutting.
const HOLD_TIME: f32 = 5.0;
/// Aim point height above the target's origin.
const LOOK_HEIGHT: f32 = 0.0;
/// False advances only on N.
const AUTO_CYCLE: bool = true;

/// A camera position relative to the target.
struct Shot {
    /// Compass angle around the target in degrees; 0 is +Z.
    angle: f32,
    /// Horizontal distance from the target.
    dist: f32,
    height: f32,
    /// Orbit speed during the shot in degrees per second; the sign is the direction.
    pan: f32,
}

const SHOTS: [Shot; 4] = [
    // High wide establishing shot.
    Shot { angle: 0.0, dist: 30.0, height: 24.0, pan: 6.0 },
    // Lower banked sweep.
    Shot { angle: 100.0, dist: 18.0, height: 13.0, pan: -8.0 },
    // Top-down bird's eye.
    Shot { angle: 200.0, dist: 40.0, height: 34.0, pan: 4.0 },
    // Close fast orbit.
    Shot { angle: 300.0, dist: 14.0, height: 9.0, pan: 12.0 },
];

#[derive(Component, Default)]
struct Director {
    shot: usize,
    /// Seconds spent in the current shot.
    timer: f32,
    /// Degrees panned within the current shot.
    orbit: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Director>() {
        ctx.insert(Director::default());
    }
    let Some(target) = find_named(ctx.world(), TARGET) else { return };
    let Some(target_pos) = ctx.get_on::<Transform>(target).map(|t| t.translation) else { return };
    let next_pressed = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(KeyCode::KeyN));

    let Some(mut director) = ctx.get_mut::<Director>() else { return };
    director.timer += dt;
    director.orbit += SHOTS[director.shot].pan * dt;
    if (AUTO_CYCLE && director.timer >= HOLD_TIME) || next_pressed {
        director.shot = (director.shot + 1) % SHOTS.len();
        director.timer = 0.0;
        director.orbit = 0.0;
    }
    let shot = &SHOTS[director.shot];
    let angle = (shot.angle + director.orbit).to_radians();
    drop(director);

    let position = target_pos + Vec3::new(angle.sin() * shot.dist, shot.height, angle.cos() * shot.dist);
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation = position;
    transform.look_at(target_pos + Vec3::Y * LOOK_HEIGHT, Vec3::Y);
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
