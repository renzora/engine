//! A swinging door driven by an eased angle rather than an animation clip, so
//! it works on any mesh. A 0..1 progress is smoothstep-eased and mapped to the
//! open angle; the creak plays once, on the open or close edge.
//!
//! Setup: attach to a door hinged about its own origin; E toggles it.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

/// Degrees of yaw when fully open.
const OPEN_ANGLE: f32 = 95.0;
/// Seconds for a full swing.
const OPEN_TIME: f32 = 0.8;
/// Yaw in degrees when closed.
const CLOSED_YAW: f32 = 0.0;

#[derive(Component, Default)]
struct Door {
    /// 0 is closed, 1 is open.
    progress: f32,
    open: bool,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Door>() {
        ctx.insert(Door::default());
    }
    let toggle = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(KeyCode::KeyE));

    let Some(mut door) = ctx.get_mut::<Door>() else { return };
    if toggle {
        door.open = !door.open;
    }
    let open = door.open;
    let dir = if open { 1.0 } else { -1.0 };
    let before = door.progress;
    door.progress = (before + dir * (dt / OPEN_TIME)).clamp(0.0, 1.0);
    let t = door.progress;
    drop(door);

    if toggle {
        let me = ctx.entity();
        ScriptAction::new("play_sound", me)
            .arg("path", if open { "door_open" } else { "door_close" })
            .arg("volume", 1.0)
            .arg("looping", false)
            .arg("bus", "Sfx")
            .send(ctx.world());
    }
    // Fully settled: leave the transform alone.
    if t == before {
        return;
    }

    let eased = t * t * (3.0 - 2.0 * t);
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.rotation = Quat::from_rotation_y((CLOSED_YAW + eased * OPEN_ANGLE).to_radians());
    }
}

renzora::script!(update);
