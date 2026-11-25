//! Binds named camera presets to number keys so a level can cut between
//! framed shots (overview, action, closeup). The cut happens on the key edge
//! only; re-issuing it every frame would fight any camera smoothing.
//!
//! Setup: attach to the camera, and capture `CameraPresets` on it named as in
//! `PRESETS`. Keys 1/2/3 pick a shot; the gamepad shoulders (L1/R1) cycle.

use bevy::prelude::*;
use renzora::core::CameraPresets;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

const PRESETS: [&str; 3] = ["overview", "action", "closeup"];

#[derive(Component, Default)]
struct Switcher {
    index: usize,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<Switcher>() {
        ctx.insert(Switcher::default());
    }
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return };
    let direct = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .iter()
        .position(|key| input.is_key_just_pressed(*key));
    let (next, prev) = match input.first_gamepad() {
        Some(pad) => (
            input.is_gamepad_button_just_pressed(pad, GamepadButton::RightTrigger),
            input.is_gamepad_button_just_pressed(pad, GamepadButton::LeftTrigger),
        ),
        None => (false, false),
    };
    let Some(current) = ctx.get::<Switcher>().map(|s| s.index) else { return };

    let n = PRESETS.len();
    let target = if let Some(i) = direct {
        Some(i)
    } else if next {
        Some((current + 1) % n)
    } else if prev {
        Some((current + n - 1) % n)
    } else {
        None
    };
    if let Some(i) = target {
        jump_to(ctx, i);
    }
}

fn jump_to(ctx: &mut ScriptCtx, index: usize) {
    let name = PRESETS[index];
    if let Some(mut switcher) = ctx.get_mut::<Switcher>() {
        switcher.index = index;
    }
    let Some(pose) = ctx.get::<CameraPresets>().and_then(|p| p.get(name).map(|pr| pr.to_transform())) else {
        renzora::clog_warn!("Script", "no camera preset named \"{name}\" on this entity");
        return;
    };
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = pose.translation;
        transform.rotation = pose.rotation;
    }
    let me = ctx.entity();
    ScriptAction::new("play_sound", me)
        .arg("path", "camera_cut")
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(ctx.world());
}

renzora::script!(update);
