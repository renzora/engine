//! A linear cutscene sequencer. Each beat sets up its shot, then arms a
//! one-shot countdown; when it runs out the step index advances. Driving it
//! from an index and a countdown (rather than sleeping) keeps it frame
//! accurate and skippable: Escape jumps straight to the end.
//!
//! Skipping also stops the sequence. The Lua original left its beat timer
//! running, so a skipped cutscene could carry on cutting afterwards.
//!
//! Setup: attach to the camera, with `CameraPresets` on it named
//! "establishing", "hero_closeup", "villain_closeup", "wide_action" and
//! "gameplay". A UI text named "Subtitle" shows the caption.

use bevy::prelude::*;
use renzora::core::CameraPresets;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

#[derive(Component)]
struct Cutscene {
    step: u32,
    /// Seconds left in the current beat.
    wait: f32,
    running: bool,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let skip = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(KeyCode::Escape));
    let Some(mut cutscene) = ctx.get_mut::<Cutscene>() else { return };
    if !cutscene.running {
        return;
    }
    if skip {
        drop(cutscene);
        finish(ctx);
        return;
    }
    cutscene.wait -= dt;
    let due = cutscene.wait <= 0.0;
    drop(cutscene);
    if due {
        advance(ctx);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        ctx.insert(Cutscene { step: 0, wait: 0.0, running: true });
        advance(ctx);
    }
}

/// Each step is one beat: set up the shot, then wait N seconds.
fn advance(ctx: &mut ScriptCtx) {
    let Some(mut cutscene) = ctx.get_mut::<Cutscene>() else { return };
    cutscene.step += 1;
    let step = cutscene.step;
    drop(cutscene);
    let me = ctx.entity();

    let wait = match step {
        1 => {
            goto_preset(ctx, "establishing");
            ScriptAction::new("play_music", me)
                .arg("path", "cutscene_theme")
                .arg("volume", 1.0)
                .arg("fade_in", 0.0)
                .arg("bus", "Music")
                .send(ctx.world());
            3.0
        }
        2 => {
            goto_preset(ctx, "hero_closeup");
            play_sound(ctx, "hero_line");
            4.0
        }
        3 => {
            goto_preset(ctx, "villain_closeup");
            play_sound(ctx, "villain_line");
            4.0
        }
        4 => {
            goto_preset(ctx, "wide_action");
            subtitle(ctx, "The battle begins!");
            2.5
        }
        _ => {
            finish(ctx);
            return;
        }
    };
    if let Some(mut cutscene) = ctx.get_mut::<Cutscene>() {
        cutscene.wait = wait;
    }
}

fn finish(ctx: &mut ScriptCtx) {
    if let Some(mut cutscene) = ctx.get_mut::<Cutscene>() {
        cutscene.running = false;
    }
    let me = ctx.entity();
    ScriptAction::new("stop_music", me).arg("fade_out", 0.0).send(ctx.world());
    subtitle(ctx, "");
    goto_preset(ctx, "gameplay");
}

fn goto_preset(ctx: &mut ScriptCtx, name: &str) {
    let Some(pose) = ctx.get::<CameraPresets>().and_then(|p| p.get(name).map(|pr| pr.to_transform())) else {
        renzora::clog_warn!("Script", "no camera preset named \"{name}\" on this entity");
        return;
    };
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.translation = pose.translation;
        transform.rotation = pose.rotation;
    }
}

fn subtitle(ctx: &mut ScriptCtx, text: &str) {
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", "Subtitle").arg("text", text).send(ctx.world());
}

fn play_sound(ctx: &mut ScriptCtx, path: &str) {
    let me = ctx.entity();
    ScriptAction::new("play_sound", me)
        .arg("path", path)
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(ctx.world());
}

renzora::script!(update, hooks = hooks);
