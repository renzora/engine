//! Pulls fog in as the entity moves away from the world origin: a cheap way to
//! build dread or focus around a landmark. The fog is only rewritten when its
//! start distance actually changes, not every frame.
//!
//! The Lua version called `set_fog`, which did nothing. This sets Bevy's
//! `DistanceFog` on the active 3D camera directly, adding one if it has none.
//!
//! Setup: attach to the player. The scene needs an active 3D camera.

use bevy::prelude::*;
use renzora::ScriptCtx;

/// Fog start distance when standing at the origin.
const NEAR: f32 = 5.0;
/// Fog end distance.
const FAR: f32 = 40.0;
/// The start never closes in nearer than this.
const MIN_START: f32 = 0.5;

#[derive(Component)]
struct FogBand {
    /// The start distance last written; negative before the first write.
    last_start: f32,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<FogBand>() {
        ctx.insert(FogBand { last_start: -1.0 });
    }
    let Some(pos) = ctx.get::<Transform>().map(|t| t.translation) else { return };
    let Some(last_start) = ctx.get::<FogBand>().map(|f| f.last_start) else { return };

    // Farther from the origin: the fog start shrinks toward the player.
    let dist = Vec2::new(pos.x, pos.z).length();
    let start = (NEAR - dist * 0.1).clamp(MIN_START, NEAR);
    if (start - last_start).abs() <= 0.05 {
        return;
    }

    let world = ctx.world();
    let mut cameras = world.query_filtered::<(Entity, &Camera), With<Camera3d>>();
    let Some(camera) = cameras.iter(world).find(|(_, c)| c.is_active).map(|(e, _)| e) else { return };
    let falloff = FogFalloff::Linear { start, end: FAR };
    if let Some(mut fog) = world.get_mut::<DistanceFog>(camera) {
        fog.falloff = falloff;
    } else {
        world.entity_mut(camera).insert(DistanceFog { falloff, ..default() });
    }

    if let Some(mut band) = ctx.get_mut::<FogBand>() {
        band.last_start = start;
    }
}

renzora::script!(update);
