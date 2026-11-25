//! Mouse-wheel zoom. The wheel nudges a clamped distance, and the entity is
//! placed at that distance along world +Z, looking at the origin.
//!
//! Setup: attach to a camera; scroll to zoom within the clamp range.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

const START_ZOOM: f32 = 8.0;
const MIN_ZOOM: f32 = 2.0;
const MAX_ZOOM: f32 = 20.0;
/// Units per wheel notch.
const SENSITIVITY: f32 = 1.5;

#[derive(Component)]
struct Zoom {
    distance: f32,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<Zoom>() {
        ctx.insert(Zoom { distance: START_ZOOM });
    }
    let scroll = ctx
        .get_resource::<ScriptInput>()
        .map(|input| input.scroll_delta.y)
        .unwrap_or(0.0);
    if scroll == 0.0 {
        return;
    }
    let Some(mut zoom) = ctx.get_mut::<Zoom>() else { return };
    // Scrolling up (positive) zooms in, to a smaller distance.
    zoom.distance = (zoom.distance - scroll * SENSITIVITY).clamp(MIN_ZOOM, MAX_ZOOM);
    let distance = zoom.distance;
    drop(zoom);

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation.z = distance;
    transform.look_at(Vec3::ZERO, Vec3::Y);
}

renzora::script!(update);
