//! A finite-state enemy: idle, patrol and chase. Transitions are driven purely
//! by distance to the player. Chase gives up at one and a half times the sight
//! range, so an enemy at the edge of sight does not flicker between states.
//!
//! Setup: attach to an enemy with "idle", "walk" and "run" clips. The player is
//! the entity named "player"; set `HOME` to the patrol anchor.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;

/// Patrol anchor on the XZ plane, as (x, z).
const HOME: Vec2 = Vec2::new(0.0, 0.0);
const PATROL_RADIUS: f32 = 4.0;
const SIGHT_RANGE: f32 = 8.0;
/// Chase speed; patrol walks at half of it.
const SPEED: f32 = 2.0;
/// Seconds idling before picking a new patrol point.
const IDLE_TIME: f32 = 2.0;
/// A patrol point counts as reached inside this radius.
const ARRIVE: f32 = 0.5;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Idle,
    Patrol,
    Chase,
}

#[derive(Component)]
struct Enemy {
    mode: Mode,
    idle_timer: f32,
    patrol_target: Vec2,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let elapsed = ctx.elapsed();
    let me = ctx.entity();
    if !ctx.has::<Enemy>() {
        // Seed a patrol point so chase-to-patrol always has somewhere to go.
        ctx.insert(Enemy { mode: Mode::Idle, idle_timer: 0.0, patrol_target: HOME + Vec2::X * PATROL_RADIUS });
        play_animation(ctx, "idle");
    }

    let Some(pos) = ctx.get::<Transform>().map(|t| t.translation) else { return };
    let here = Vec2::new(pos.x, pos.z);
    let player = find_named(ctx.world(), "player")
        .and_then(|p| ctx.get_on::<Transform>(p))
        .map(|t| Vec2::new(t.translation.x, t.translation.z))
        .unwrap_or(here);
    let to_player = here.distance(player);

    let Some(mut enemy) = ctx.get_mut::<Enemy>() else { return };
    let mut crossfade: Option<(&str, f32)> = None;
    let mut steer: Option<(Vec2, f32)> = None;
    match enemy.mode {
        Mode::Idle => {
            enemy.idle_timer += dt;
            if to_player < SIGHT_RANGE {
                enemy.mode = Mode::Chase;
                crossfade = Some(("run", 0.2));
            } else if enemy.idle_timer > IDLE_TIME {
                enemy.idle_timer = 0.0;
                // A new point on the ring; elapsed time makes it vary per visit.
                let a = elapsed * 1.3;
                enemy.patrol_target = HOME + Vec2::new(a.cos(), a.sin()) * PATROL_RADIUS;
                enemy.mode = Mode::Patrol;
                crossfade = Some(("walk", 0.25));
            }
        }
        Mode::Patrol => {
            if to_player < SIGHT_RANGE {
                enemy.mode = Mode::Chase;
                crossfade = Some(("run", 0.2));
            } else if here.distance(enemy.patrol_target) < ARRIVE {
                enemy.mode = Mode::Idle;
                crossfade = Some(("idle", 0.25));
            } else {
                steer = Some((enemy.patrol_target, SPEED * 0.5));
            }
        }
        Mode::Chase => {
            if to_player > SIGHT_RANGE * 1.5 {
                enemy.mode = Mode::Idle;
                crossfade = Some(("idle", 0.3));
            } else {
                steer = Some((player, SPEED));
            }
        }
    }
    drop(enemy);

    if let Some((target, speed)) = steer {
        move_towards(ctx, target, speed * dt);
    }
    if let Some((name, duration)) = crossfade {
        ScriptAction::new("crossfade_animation", me)
            .arg("name", name)
            .arg("duration", duration)
            .arg("looping", true)
            .send(ctx.world());
    }
}

/// Steps toward a point on the XZ plane and turns to face it.
fn move_towards(ctx: &mut ScriptCtx, target: Vec2, step: f32) {
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    let offset = target - Vec2::new(transform.translation.x, transform.translation.z);
    let len = offset.length();
    if len <= 0.001 {
        return;
    }
    let dir = offset / len;
    transform.translation += Vec3::new(dir.x, 0.0, dir.y) * step;
    let y = transform.translation.y;
    transform.look_at(Vec3::new(target.x, y, target.y), Vec3::Y);
}

fn play_animation(ctx: &mut ScriptCtx, name: &str) {
    let me = ctx.entity();
    ScriptAction::new("play_animation", me)
        .arg("name", name)
        .arg("looping", true)
        .arg("speed", 1.0)
        .send(ctx.world());
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
