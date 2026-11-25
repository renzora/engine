//! A ragdoll trigger demo. A couple of seconds after Play starts the character
//! collapses on its own; the recover key stands it back up.
//!
//! Ragdoll is a physics feature, so it only does anything in Play (the editor
//! pauses the simulation). The drop waits rather than firing on the first
//! frame because the model's skeleton and its physics bodies spawn a few
//! frames after the scene loads, and triggering earlier would do nothing.
//!
//! Setup: attach to the entity that carries the `Ragdoll` component (the model
//! root, beside `Animator`); the ragdoll actions act on the script's own
//! entity.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

/// Seconds after Play before the ragdoll fires.
const DROP_DELAY: f32 = 2.0;
const RECOVER_KEY: KeyCode = KeyCode::KeyG;

/// Each transition fires once, on the edge.
#[derive(Component, Default)]
struct Ragdoll {
    ragdolling: bool,
    waited: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let recover = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(RECOVER_KEY));
    let Some(mut state) = ctx.get_mut::<Ragdoll>() else { return };
    let action = if !state.ragdolling {
        state.waited += dt;
        if state.waited < DROP_DELAY {
            return;
        }
        state.ragdolling = true;
        "enable_ragdoll"
    } else if recover {
        state.ragdolling = false;
        state.waited = 0.0;
        "disable_ragdoll"
    } else {
        return;
    };
    drop(state);

    let me = ctx.entity();
    ScriptAction::new(action, me).send(ctx.world());
    if action == "enable_ragdoll" {
        renzora::clog_info!("Script", "[ragdoll_demo] ragdoll enabled");
    } else {
        renzora::clog_info!("Script", "[ragdoll_demo] ragdoll disabled (animation resumed)");
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if matches!(hook, ScriptHook::Ready) {
        ctx.insert(Ragdoll::default());
        renzora::clog_info!(
            "Script",
            "[ragdoll_demo] Play mode: collapsing in {DROP_DELAY}s; press {RECOVER_KEY:?} to recover"
        );
    }
}

renzora::script!(update, hooks = hooks);
