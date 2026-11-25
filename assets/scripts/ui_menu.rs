//! A menu authored in markup, with its callbacks handled here. The buttons in
//! `ui/example_menu.html` carry `action="start_game"` and so on; those names
//! have no engine binding, so the markup system forwards them to every
//! script's `Ui` hook (a broadcast, like an RPC).
//!
//! Setup: attach to ONE entity. It spawns `ui/example_menu.html` on its first
//! frame.

use bevy::prelude::*;
use renzora::script_bridge::{ScriptAction, ScriptActionValue};
use renzora::{ScriptCtx, ScriptHook};

const TEMPLATE: &str = "ui/example_menu.html";

fn update(_ctx: &mut ScriptCtx) {}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            let me = ctx.entity();
            ScriptAction::new("hui_spawn", me).arg("template", TEMPLATE).send(ctx.world());
        }
        ScriptHook::Ui { name, args, .. } => match *name {
            "start_game" => {
                // `tag:scene="level_01"` on the button arrives as `scene`.
                let scene = match args.get("scene") {
                    Some(ScriptActionValue::String(s)) => s.as_str(),
                    _ => "nil",
                };
                renzora::clog_info!("Script", "Start game -> scene {scene}");
            }
            "open_settings" => renzora::clog_info!("Script", "Open settings"),
            "open_credits" => renzora::clog_info!("Script", "Open credits"),
            "quit_game" => renzora::clog_info!("Script", "Quit requested"),
            _ => {}
        },
        _ => {}
    }
}

renzora::script!(update, hooks = hooks);
