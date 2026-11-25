//! Patrols a loop of waypoints on the XZ plane, advancing on arrival and
//! wrapping around for an endless loop.
//!
//! Setup: attach to the patroller and edit `WAYPOINTS`.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Waypoints as (x, z), walked in order and then from the first again.
const WAYPOINTS: [(f32, f32); 4] = [(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0)];
/// Units per second.
const SPEED: f32 = 3.0;
/// A waypoint counts as reached inside this radius.
const ARRIVE: f32 = 0.2;

#[derive(Component)]
struct Patrol {
    index: usize,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Patrol>() {
        ctx.insert(Patrol { index: 0 });
    }
    let Some(index) = ctx.get::<Patrol>().map(|p| p.index) else { return };
    let (wx, wz) = WAYPOINTS[index % WAYPOINTS.len()];

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    let offset = Vec2::new(wx - transform.translation.x, wz - transform.translation.z);
    let dist = offset.length();
    if dist > ARRIVE {
        let dir = offset / dist;
        transform.translation += Vec3::new(dir.x, 0.0, dir.y) * SPEED * dt;
        return;
    }
    drop(transform);
    if let Some(mut patrol) = ctx.get_mut::<Patrol>() {
        patrol.index = (index + 1) % WAYPOINTS.len();
    }
}

renzora::script!(update);
