//! Shows the live camera EV-100 that auto-exposure's GPU luminance readback
//! reports, refreshed on an interval so the number does not jitter. It reads
//! about 0.0 until auto-exposure has run (first frame, auto-exposure disabled,
//! or no readback complete yet).
//!
//! Setup: attach to a UI text widget; it rewrites its own text.

use bevy::prelude::*;
use renzora::core::components::CameraExposureState;
use renzora::{ScriptCtx, ScriptHook};

/// Seconds between text refreshes.
const UPDATE_INTERVAL: f32 = 0.1;
/// Prefix shown before the number.
const LABEL: &str = "EV: ";

#[derive(Component)]
struct Refresh(f32);

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut refresh) = ctx.get_mut::<Refresh>() else { return };
    refresh.0 += dt;
    if refresh.0 < UPDATE_INTERVAL {
        return;
    }
    refresh.0 = 0.0;
    drop(refresh);
    let ev = ctx.get_resource::<CameraExposureState>().map_or(0.0, |s| s.ev100);
    set_text(ctx, format!("{LABEL}{ev:.2}"));
}

fn set_text(ctx: &mut ScriptCtx, value: String) {
    if let Some(mut text) = ctx.get_mut::<Text>() {
        text.0 = value;
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        ctx.insert(Refresh(0.0));
        set_text(ctx, format!("{LABEL}--"));
    }
}

renzora::script!(update, hooks = hooks);
