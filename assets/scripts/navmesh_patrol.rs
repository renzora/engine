//! Waypoint patrol on a baked navmesh. The agent is handed one destination at
//! a time and the script watches for arrival, then advances the index. Letting
//! the navmesh do the pathfinding keeps the script free of steering maths.
//!
//! Arrival prefers the nav mirror's own remaining distance
//! (`NavReadState.distance_to_destination`) while the agent has a target, and
//! falls back to the planar distance to the waypoint otherwise.
//!
//! Setup: bake a navmesh, attach to a nav agent, and edit `WAYPOINTS`.

use bevy::prelude::*;
use renzora::core::reflection::get_reflected_field;
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::{ScriptCtx, ScriptHook};

/// The route, visited in order and looped.
const WAYPOINTS: [Vec3; 4] = [
    Vec3::new(5.0, 0.0, 0.0),
    Vec3::new(5.0, 0.0, 5.0),
    Vec3::new(-5.0, 0.0, 5.0),
    Vec3::new(-5.0, 0.0, -5.0),
];
/// How close counts as having reached a waypoint.
const ARRIVE_DIST: f32 = 0.6;

#[derive(Component)]
struct Patrol {
    index: usize,
}

fn update(ctx: &mut ScriptCtx) {
    let Some(index) = ctx.get::<Patrol>().map(|p| p.index) else { return };
    let me = ctx.entity();
    let waypoint = WAYPOINTS[index];

    let has_target = matches!(
        get_reflected_field(ctx.world(), me, "NavReadState", "has_target"),
        Some(PropertyValue::Bool(true))
    );
    let remaining = match get_reflected_field(ctx.world(), me, "NavReadState", "distance_to_destination") {
        Some(PropertyValue::Float(d)) if has_target => Some(d),
        _ => None,
    };
    let arrived = match remaining {
        Some(d) => d <= ARRIVE_DIST,
        None => {
            let Some(transform) = ctx.get::<Transform>() else { return };
            let d = (waypoint - transform.translation).xz();
            d.length_squared() <= ARRIVE_DIST * ARRIVE_DIST
        }
    };

    if arrived {
        let next = (index + 1) % WAYPOINTS.len();
        if let Some(mut patrol) = ctx.get_mut::<Patrol>() {
            patrol.index = next;
        }
        go_to(ctx, next);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        ctx.insert(Patrol { index: 0 });
        go_to(ctx, 0);
    }
}

fn go_to(ctx: &mut ScriptCtx, index: usize) {
    let me = ctx.entity();
    ScriptAction::new("nav_set_destination", me)
        .arg("target", WAYPOINTS[index])
        .send(ctx.world());
}

renzora::script!(update, hooks = hooks);
