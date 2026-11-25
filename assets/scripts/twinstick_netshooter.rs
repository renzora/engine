//! A twin-stick shooter: move with the left stick, aim and fire with the right.
//! Aim comes from the right stick's direction, and each shot is both spawned
//! locally and broadcast as an RPC so remote peers see your bullets. Firing is
//! gated on a cooldown, so holding the stick out fires at a fixed cadence.
//!
//! As in the Lua version, a bullet is only a marker where the shot left the
//! barrel: nothing moves it afterwards.
//!
//! Setup: attach to the player pawn; needs a gamepad. Connect first for the
//! networked part: remote shots arrive through the `shoot` RPC and spawn
//! bullets named for their sender (`blt_p<peer>_<n>`, local ones `blt_me_<n>`).

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::network::{self as net, Rpc, RpcArgs};
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const SPEED: f32 = 6.0;
/// Seconds between shots while aiming.
const COOLDOWN: f32 = 0.18;
const MOVE_DEADZONE: f32 = 0.15;
const AIM_DEADZONE: f32 = 0.3;
const BULLET_SIZE: f32 = 0.3;

#[derive(Component, Default)]
struct Shooter {
    cooldown: f32,
    /// Shots spawned so far, local and remote, so every bullet name is unique.
    shot: u32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let me = ctx.entity();
    if !ctx.has::<Shooter>() {
        ctx.insert(Shooter::default());
    }
    let (left, right) = match ctx.get_resource::<ScriptInput>() {
        Some(input) => match input.first_gamepad() {
            Some(pad) => (input.get_gamepad_left_stick(pad), input.get_gamepad_right_stick(pad)),
            None => (Vec2::ZERO, Vec2::ZERO),
        },
        None => (Vec2::ZERO, Vec2::ZERO),
    };

    // Stick up is screen forward, which is -Z.
    if left.x.abs() > MOVE_DEADZONE || left.y.abs() > MOVE_DEADZONE {
        ScriptAction::new("kinematic_slide", me)
            .arg("x", left.x * SPEED * dt)
            .arg("y", 0.0)
            .arg("z", -left.y * SPEED * dt)
            .send(ctx.world());
    }

    let Some(mut shooter) = ctx.get_mut::<Shooter>() else { return };
    shooter.cooldown = (shooter.cooldown - dt).max(0.0);
    let ready = shooter.cooldown <= 0.0;
    drop(shooter);

    if right.length() <= AIM_DEADZONE {
        return;
    }
    let dir = Vec2::new(right.x, -right.y).normalize();
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    // Face along the aim: -Z is forward, so the yaw that maps -Z onto it.
    transform.rotation = Quat::from_rotation_y(f32::atan2(-dir.x, -dir.y));
    let pos = transform.translation;
    drop(transform);
    if !ready {
        return;
    }
    if let Some(mut shooter) = ctx.get_mut::<Shooter>() {
        shooter.cooldown = COOLDOWN;
    }

    spawn_bullet(ctx, pos.x + dir.x, pos.z + dir.y, "me");
    ScriptAction::new("play_sound", me)
        .arg("path", "sfx/shoot.ogg")
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(ctx.world());
    if net::is_connected(ctx.world()) {
        Rpc::new("shoot")
            .arg("x", pos.x)
            .arg("z", pos.z)
            .arg("dx", dir.x)
            .arg("dz", dir.y)
            .send(ctx.world());
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    let ScriptHook::Rpc { name: "shoot", args, from } = hook else { return };
    if *from == 0 {
        return;
    }
    let (Some(x), Some(z), Some(dx), Some(dz)) = (args.f32("x"), args.f32("z"), args.f32("dx"), args.f32("dz")) else {
        return;
    };
    if !ctx.has::<Shooter>() {
        ctx.insert(Shooter::default());
    }
    spawn_bullet(ctx, x + dx, z + dz, &format!("p{from}"));
}

fn spawn_bullet(ctx: &mut ScriptCtx, x: f32, z: f32, tag: &str) {
    let Some(mut shooter) = ctx.get_mut::<Shooter>() else { return };
    shooter.shot += 1;
    let name = format!("blt_{tag}_{}", shooter.shot);
    drop(shooter);
    // Lua shrank only the X axis, which left a flat disc; the bullet is meant
    // to be a small ball.
    ctx.world().spawn((
        Name::new(name),
        Transform::from_xyz(x, 1.0, z).with_scale(Vec3::splat(BULLET_SIZE)),
        Visibility::default(),
        MeshPrimitive("sphere".into()),
        MeshColor(Color::srgb(1.0, 0.9, 0.2)),
    ));
}

renzora::script!(update, hooks = hooks);
