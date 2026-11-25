//! A state-replicated elevator: a call, travel and doors FSM whose state is
//! mirrored to every peer so everyone rides the same car. The car's height must
//! be identical for all players, so the server runs the FSM and broadcasts the
//! target floor, phase and height; clients only ease the car toward the last
//! authoritative height.
//!
//! Floors are numbered from 1 on the wire (`elev_call` with `floor`).
//!
//! Setup: attach to the elevator platform; `FLOORS` lists the Y heights.
//! Clients request a floor with an `elev_call` RPC to the server. On the host,
//! PageUp / PageDown call the next floor for testing.

use bevy::prelude::*;
use renzora::network::{self as net, Rpc, RpcArgs};
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const FLOORS: [f32; 4] = [0.0, 5.0, 10.0, 15.0];
/// Seconds the doors stay open on arrival.
const DOOR_TIME: f32 = 3.0;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Moving,
    Doors,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Moving => "moving",
            Phase::Doors => "doors",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "idle" => Some(Phase::Idle),
            "moving" => Some(Phase::Moving),
            "doors" => Some(Phase::Doors),
            _ => None,
        }
    }
}

#[derive(Component)]
struct Elevator {
    /// Floor indices, 0-based here.
    cur: usize,
    target: usize,
    phase: Phase,
    car_y: f32,
    doors_left: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let me = ctx.entity();
    let world = ctx.world();
    let pure_client = net::is_client(world) && !net::is_server(world);
    let Some(car_y) = ctx.get::<Elevator>().map(|e| e.car_y) else { return };

    if pure_client {
        if let Some(mut t) = ctx.get_mut::<Transform>() {
            let y = t.translation.y;
            t.translation.y = y + (car_y - y) * (dt * 3.0).clamp(0.0, 1.0);
        }
        return;
    }

    let (up, down) = match ctx.get_resource::<ScriptInput>() {
        Some(i) => (i.is_key_just_pressed(KeyCode::PageUp), i.is_key_just_pressed(KeyCode::PageDown)),
        None => (false, false),
    };

    let Some(mut e) = ctx.get_mut::<Elevator>() else { return };
    let mut send = false;
    let mut ding = false;
    if e.phase == Phase::Moving {
        let goal = FLOORS[e.target];
        e.car_y += (goal - e.car_y) * (dt * 1.5).clamp(0.0, 1.0);
        if (e.car_y - goal).abs() < 0.05 {
            e.car_y = goal;
            e.cur = e.target;
            e.phase = Phase::Doors;
            e.doors_left = DOOR_TIME;
            ding = true;
        }
        send = true;
    }
    if e.phase == Phase::Doors && !ding {
        e.doors_left -= dt;
        if e.doors_left <= 0.0 {
            e.phase = Phase::Idle;
            send = true;
        }
    }
    if e.phase == Phase::Idle {
        if up && e.cur + 1 < FLOORS.len() {
            e.target = e.cur + 1;
            e.phase = Phase::Moving;
            send = true;
        } else if down && e.cur > 0 {
            e.target = e.cur - 1;
            e.phase = Phase::Moving;
            send = true;
        }
    }
    let car_y = e.car_y;
    drop(e);

    if let Some(mut t) = ctx.get_mut::<Transform>() {
        t.translation.y = car_y;
    }
    if ding {
        ScriptAction::new("play_sound", me)
            .arg("path", "sfx/elev_ding.ogg")
            .arg("volume", 1.0)
            .arg("looping", false)
            .arg("bus", "Sfx")
            .send(ctx.world());
    }
    if send {
        broadcast(ctx);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            let car_y = FLOORS[0];
            ctx.insert(Elevator { cur: 0, target: 0, phase: Phase::Idle, car_y, doors_left: 0.0 });
            if let Some(mut t) = ctx.get_mut::<Transform>() {
                t.translation.y = car_y;
            }
        }
        ScriptHook::Rpc { name: "elev_call", args, .. } => {
            if !net::is_server(ctx.world()) {
                return;
            }
            let Some(mut e) = ctx.get_mut::<Elevator>() else { return };
            if e.phase != Phase::Idle {
                return;
            }
            let floor = args.f32("floor").map(|f| f.floor() as i64).unwrap_or(e.cur as i64 + 1);
            if floor < 1 || floor > FLOORS.len() as i64 || floor as usize - 1 == e.cur {
                return;
            }
            e.target = floor as usize - 1;
            e.phase = Phase::Moving;
            drop(e);
            broadcast(ctx);
        }
        ScriptHook::Rpc { name: "elev", args, .. } => {
            if !net::is_client(ctx.world()) {
                return;
            }
            let (Some(target), Some(phase), Some(y)) =
                (args.i64("target"), args.str("phase").and_then(Phase::parse), args.f32("y"))
            else {
                return;
            };
            if let Some(mut e) = ctx.get_mut::<Elevator>() {
                e.target = (target.max(1) as usize - 1).min(FLOORS.len() - 1);
                e.phase = phase;
                e.car_y = y;
            }
        }
        _ => {}
    }
}

fn broadcast(ctx: &mut ScriptCtx) {
    let Some(e) = ctx.get::<Elevator>() else { return };
    let rpc = Rpc::new("elev")
        .arg("target", e.target as i64 + 1)
        .arg("phase", e.phase.as_str())
        .arg("y", e.car_y);
    rpc.send(ctx.world());
}

renzora::script!(update, hooks = hooks);
