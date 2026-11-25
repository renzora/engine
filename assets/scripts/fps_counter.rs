//! An FPS counter. Writes the frame rate into this entity's own Text each
//! refresh, averaged over a short window so the number does not strobe.
//!
//! Setup: attach to a UI text widget.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};

/// Seconds between text refreshes.
const UPDATE_INTERVAL: f32 = 0.25;
/// Shown before the number.
const LABEL: &str = "FPS: ";

#[derive(Component, Default)]
struct FpsWindow {
    /// Seconds since the last refresh.
    time: f32,
    /// Frames since the last refresh.
    frames: u32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut window) = ctx.get_mut::<FpsWindow>() else { return };
    window.time += dt;
    window.frames += 1;
    if window.time < UPDATE_INTERVAL {
        return;
    }
    let fps = window.frames as f32 / window.time;
    *window = FpsWindow::default();
    drop(window);
    set_text(ctx, format!("{LABEL}{fps:.0}"));
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        ctx.insert(FpsWindow::default());
        set_text(ctx, format!("{LABEL}--"));
    }
}

fn set_text(ctx: &mut ScriptCtx, value: String) {
    if let Some(mut text) = ctx.get_mut::<Text>() {
        text.0 = value;
    }
}

renzora::script!(update, hooks = hooks);
