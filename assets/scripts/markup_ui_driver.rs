//! Drives a markup menu panel: spawn a template, react to its buttons in the
//! `Ui` hook. Markup widgets send named callbacks back to the script, and the
//! script is the controller that owns the state and pushes labels with
//! `ui_set_text`: the view is markup, the logic is the script.
//!
//! Setup: a markup template "menu_panel" whose buttons fire "play", "options"
//! and "quit", a label named "menu_status" and a drawer named
//! "options_drawer". "play" loads `LEVEL`. Purely local UI, no networking.

use bevy::prelude::*;
use renzora::core::session::PendingSceneLoad;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook};

const TEMPLATE: &str = "menu_panel";
const STATUS: &str = "menu_status";
/// The scene "play" opens. The Lua original named a `.scene` file; scenes are
/// `.bsn` now.
const LEVEL: &str = "scenes/level1.bsn";

#[derive(Component, Default)]
struct Menu {
    clicks: u32,
}

fn update(ctx: &mut ScriptCtx) {
    let _ = ctx;
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    let me = ctx.entity();
    match hook {
        ScriptHook::Ready => {
            ctx.insert(Menu::default());
            ScriptAction::new("hui_spawn", me).arg("template", TEMPLATE).send(ctx.world());
            set_status(ctx, "Ready".to_string());
        }
        ScriptHook::Ui { name, source, .. } => {
            let Some(mut menu) = ctx.get_mut::<Menu>() else { return };
            menu.clicks += 1;
            let clicks = menu.clicks;
            drop(menu);
            match *name {
                "play" => {
                    set_status(ctx, "Starting...".to_string());
                    match ctx.get_resource_mut::<PendingSceneLoad>() {
                        Some(mut pending) => pending.requests.push(LEVEL.to_string()),
                        None => renzora::clog_warn!("Script", "no scene loader in this build"),
                    }
                }
                "options" => {
                    ScriptAction::new("ui_toggle", me).arg("name", "options_drawer").send(ctx.world());
                    set_status(ctx, format!("Options toggled ({clicks} clicks)"));
                }
                "quit" => {
                    ScriptAction::new("hui_despawn", me).arg("template", TEMPLATE).send(ctx.world());
                    ScriptAction::new("quit", me).send(ctx.world());
                }
                // An unknown callback is echoed so template authors can debug
                // their bindings.
                other => set_status(ctx, format!("event: {other} from {source:?}")),
            }
        }
        _ => {}
    }
}

fn set_status(ctx: &mut ScriptCtx, text: String) {
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", STATUS).arg("text", text).send(ctx.world());
}

renzora::script!(update, hooks = hooks);
