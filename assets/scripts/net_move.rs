//! Server-driven motion that replicates to clients. The server moves this
//! entity in a circle and its Transform replicates to every client with
//! interpolation; clients never drive it, they receive it.
//!
//! Setup: attach to an entity that exists in the loaded scene on every peer
//! (a cube placed in the scene), and give it the `Networked` component in the
//! Inspector. Test on a scene entity rather than one spawned on the server, so
//! its visual is already present on the clients.

use bevy::prelude::*;
use renzora::network as net;
use renzora::ScriptCtx;

const RADIUS: f32 = 3.0;
/// Radians per second.
const SPEED: f32 = 1.0;

fn update(ctx: &mut ScriptCtx) {
    if !net::is_server(ctx.world()) {
        return;
    }
    let a = ctx.elapsed() * SPEED;
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = Vec3::new(a.cos() * RADIUS, 1.0, a.sin() * RADIUS);
    }
}

renzora::script!(update);
