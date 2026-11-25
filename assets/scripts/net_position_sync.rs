//! Hand-rolled transform replication over RPCs. Each owner broadcasts its pose
//! on a fixed tick, and every other peer spawns a "ghost" cube per sender and
//! eases it toward the last received sample, which hides the jitter between
//! sparse updates.
//!
//! The engine does replicate transforms itself now (see the multiplayer docs);
//! this stays as the minimal example of doing it by hand with RPCs.
//!
//! Setup: attach to the locally controlled entity, then connect. Every peer
//! sees a smoothed cube named "ghost_<peer id>" for every other owner.

use std::collections::HashMap;

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::network::{self as net, Rpc, RpcArgs};
use renzora::{ScriptCtx, ScriptHook};

/// Pose broadcasts per second.
const SEND_HZ: f32 = 10.0;
/// How quickly a ghost closes on its latest sample, per second.
const SMOOTHING: f32 = 12.0;

/// A remote peer's stand-in and where it was last reported.
struct Ghost {
    entity: Entity,
    target: Vec3,
}

#[derive(Component, Default)]
struct PoseSync {
    /// Seconds until the next broadcast.
    tick: f32,
    ghosts: HashMap<u64, Ghost>,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<PoseSync>() {
        ctx.insert(PoseSync::default());
    }
    let Some(mut sync) = ctx.get_mut::<PoseSync>() else { return };
    sync.tick -= dt;
    let send = sync.tick <= 0.0;
    if send {
        sync.tick += 1.0 / SEND_HZ;
    }
    let ghosts: Vec<(Entity, Vec3)> = sync.ghosts.values().map(|g| (g.entity, g.target)).collect();
    drop(sync);

    if send && net::is_connected(ctx.world()) {
        if let Some(transform) = ctx.get::<Transform>().map(|t| *t) {
            let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            let p = transform.translation;
            Rpc::new("pose")
                .arg("x", p.x)
                .arg("y", p.y)
                .arg("z", p.z)
                .arg("yaw", yaw.to_degrees())
                .unreliable()
                .send(ctx.world());
        }
    }

    let a = (dt * SMOOTHING).clamp(0.0, 1.0);
    for (entity, target) in ghosts {
        if let Some(mut transform) = ctx.get_mut_on::<Transform>(entity) {
            transform.translation = transform.translation.lerp(target, a);
        }
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Rpc { name: "pose", args, from } => {
            if *from == 0 {
                return;
            }
            let (Some(x), Some(y), Some(z)) = (args.f32("x"), args.f32("y"), args.f32("z")) else { return };
            let target = Vec3::new(x, y, z);
            let known = ctx.get::<PoseSync>().is_some_and(|s| s.ghosts.contains_key(from));
            if known {
                if let Some(mut sync) = ctx.get_mut::<PoseSync>() {
                    if let Some(ghost) = sync.ghosts.get_mut(from) {
                        ghost.target = target;
                    }
                }
                return;
            }
            let entity = ctx
                .world()
                .spawn((
                    Name::new(format!("ghost_{from}")),
                    Transform::from_translation(target),
                    Visibility::default(),
                    MeshPrimitive("cube".into()),
                    MeshColor(Color::srgb(0.2, 0.7, 1.0)),
                ))
                .id();
            if let Some(mut sync) = ctx.get_mut::<PoseSync>() {
                sync.ghosts.insert(*from, Ghost { entity, target });
            }
        }
        ScriptHook::PlayerLeft { id } => {
            let Some(ghost) = ctx.get_mut::<PoseSync>().and_then(|mut s| s.ghosts.remove(id)) else { return };
            ctx.world().despawn(ghost.entity);
        }
        _ => {}
    }
}

renzora::script!(update, hooks = hooks);
