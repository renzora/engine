//! Forward `bevy_hui` markup callbacks into the engine's Lua scripts.
//!
//! A template event like `on_press="start_game"` resolves through bevy_hui's
//! [`FunctionBindings`]: if `start_game` is bound to a Rust one-shot system it
//! runs that; otherwise bevy_hui logs "function not bound". To make markup
//! callbacks drive *script* logic instead, we register a fallback forwarder for
//! every callback name that has no Rust binding. The forwarder pushes a
//! [`renzora::UiCallback`] into [`renzora::ScriptUiInbox`], which
//! `renzora_scripting` drains each frame into every script's
//! `on_ui(name, args, entity)` hook (broadcast, same as `on_rpc`).
//!
//! Precedence: a name already present in [`FunctionBindings`] (a real Rust
//! binding) is left untouched, so Rust handlers win and Lua is the fallback.
//!
//! Only "action-like" events forward to scripts: `on_press`, `on_change`,
//! `on_spawn`. Hover (`on_enter` / `on_exit`) is intentionally not forwarded —
//! it fires constantly and is normally handled by markup `hover:` styles.

use bevy::prelude::*;
use bevy_hui::prelude::{
    FunctionBindings, HtmlNode, OnUiChange, OnUiPress, OnUiSpawn, Tags,
};
use renzora::{ScriptAction, ScriptActionValue, ScriptUiInbox, UiCallback};

/// Registers a fallback forwarder for any markup callback name on a freshly
/// built node that isn't already bound to a Rust system. Runs every frame but
/// only touches newly-added event components (cheap `Added<…>` filter).
pub fn register_lua_forwarders(
    mut cmd: Commands,
    mut bindings: ResMut<FunctionBindings>,
    nodes: Query<
        (Option<&OnUiPress>, Option<&OnUiChange>, Option<&OnUiSpawn>),
        Or<(Added<OnUiPress>, Added<OnUiChange>, Added<OnUiSpawn>)>,
    >,
) {
    for (press, change, spawn) in &nodes {
        let lists = [
            press.map(|c| &c.0),
            change.map(|c| &c.0),
            spawn.map(|c| &c.0),
        ];
        for names in lists.into_iter().flatten() {
            for name in names {
                // Respect existing Rust bindings; only register once per name.
                if bindings.contains_key(name) {
                    continue;
                }
                let captured = name.clone();
                let id = cmd.register_system(
                    move |In(entity): In<Entity>,
                          mut inbox: ResMut<ScriptUiInbox>,
                          tags: Query<&Tags>| {
                        let mut args = std::collections::HashMap::new();
                        if let Ok(t) = tags.get(entity) {
                            for (k, v) in t.tags() {
                                args.insert(k.clone(), ScriptActionValue::String(v.clone()));
                            }
                        }
                        inbox.pending.push(UiCallback {
                            name: captured.clone(),
                            args,
                            entity_bits: entity.to_bits(),
                        });
                    },
                );
                bindings.register(name.clone(), id);
            }
        }
    }
}

/// Lets scripts spawn a markup template:
/// `action("hui_spawn", { template = "ui/example_menu.html" })`.
pub fn handle_hui_spawn(
    trigger: On<ScriptAction>,
    asset_server: Res<AssetServer>,
    mut cmd: Commands,
) {
    let action = trigger.event();
    if action.name != "hui_spawn" {
        return;
    }
    let Some(ScriptActionValue::String(path)) = action.args.get("template") else {
        warn!("hui_spawn: missing string arg `template`");
        return;
    };
    cmd.spawn(HtmlNode(asset_server.load(path.clone())));
}
