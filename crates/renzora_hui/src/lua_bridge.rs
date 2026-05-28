//! Forward markup callbacks into the engine's Lua scripts (Phase D will rebuild
//! the `on_press="…"` → Lua `on_ui` bridge on top of our own interaction
//! handler, replacing bevy_hui's `FunctionBindings`). For now, only the
//! `action("hui_spawn", …)` Lua → engine helper remains, so scripts can spawn
//! a template by path.

use bevy::prelude::*;
use renzora::{ScriptAction, ScriptActionValue};
use renzora_game_ui::HtmlTemplatePath;

/// Lets scripts spawn a markup template:
///     `action("hui_spawn", { template = "ui/example_menu.html" })`
pub fn handle_hui_spawn(trigger: On<ScriptAction>, mut cmd: Commands) {
    let action = trigger.event();
    if action.name != "hui_spawn" {
        return;
    }
    let Some(ScriptActionValue::String(path)) = action.args.get("template") else {
        warn!("hui_spawn: missing string arg `template`");
        return;
    };
    cmd.spawn(HtmlTemplatePath(path.clone()));
}
