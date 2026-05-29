//! Lua → engine actions implemented as `ScriptAction` observers.
//!
//! Scripts call these via `action("name", { ... })`. Each `action(...)` from
//! Lua fires a `ScriptAction` event; the observers below filter by `name`.
//!
//! Verbs:
//! - `hui_spawn   { template = "templates/foo.html" }` — spawn a markup tree.
//! - `hui_despawn { template = "templates/foo.html" }` — despawn every entity
//!   whose `HtmlTemplatePath` matches. Also accepts `name = "..."` to despawn
//!   by entity Name (markup `name="..."` attribute).
//! - `hui_hide    { name = "..." }` — `Visibility::Hidden` on the named entity.
//! - `hui_show    { name = "..." }` — `Visibility::Inherited` on the named entity.
//! - `quit` — send `AppExit::Success`.

use bevy::app::AppExit;
use bevy::prelude::*;
use renzora::{ScriptAction, ScriptActionValue};
use renzora_game_ui::HtmlTemplatePath;

/// `action("hui_spawn", { template = "templates/foo.html" })` — spawn a new
/// entity carrying that path; the renzora_hui loader takes over from there.
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

/// `action("hui_despawn", { template = "templates/foo.html" })`
/// or `action("hui_despawn", { name = "main_menu_root" })`.
pub fn handle_hui_despawn(
    trigger: On<ScriptAction>,
    paths: Query<(Entity, &HtmlTemplatePath)>,
    names: Query<(Entity, &Name)>,
    mut commands: Commands,
) {
    let action = trigger.event();
    if action.name != "hui_despawn" {
        return;
    }
    if let Some(ScriptActionValue::String(template)) = action.args.get("template") {
        for (entity, path) in &paths {
            if path.0 == *template {
                commands.entity(entity).despawn();
            }
        }
        return;
    }
    if let Some(ScriptActionValue::String(name)) = action.args.get("name") {
        for (entity, en) in &names {
            if en.as_str() == name {
                commands.entity(entity).despawn();
            }
        }
        return;
    }
    warn!("hui_despawn: needs either `template` or `name` arg");
}

/// `action("hui_hide", { name = "main_menu_root" })` — hide the markup
/// subtree rooted at the entity with that `Name`. Set `Visibility::Hidden`
/// rather than despawn so a later `hui_show` can flip it back without
/// triggering a rebuild.
pub fn handle_hui_hide(
    trigger: On<ScriptAction>,
    names: Query<(Entity, &Name)>,
    mut commands: Commands,
) {
    let action = trigger.event();
    if action.name != "hui_hide" {
        return;
    }
    let Some(ScriptActionValue::String(name)) = action.args.get("name") else {
        warn!("hui_hide: missing string arg `name`");
        return;
    };
    for (entity, en) in &names {
        if en.as_str() == name {
            commands.entity(entity).insert(Visibility::Hidden);
        }
    }
}

/// `action("hui_show", { name = "main_menu_root" })` — counterpart to
/// `hui_hide`. Sets `Visibility::Inherited` so the entity follows its parent
/// (or `Visibility::Visible` if it has no parent — bevy_ui treats root
/// `Inherited` as visible).
pub fn handle_hui_show(
    trigger: On<ScriptAction>,
    names: Query<(Entity, &Name)>,
    mut commands: Commands,
) {
    let action = trigger.event();
    if action.name != "hui_show" {
        return;
    }
    let Some(ScriptActionValue::String(name)) = action.args.get("name") else {
        warn!("hui_show: missing string arg `name`");
        return;
    };
    for (entity, en) in &names {
        if en.as_str() == name {
            commands.entity(entity).insert(Visibility::Inherited);
        }
    }
}

/// `action("quit")` — send `AppExit::Success`. The Bevy main loop catches it
/// next tick and closes the window / ends the process.
pub fn handle_quit(trigger: On<ScriptAction>, mut exit: MessageWriter<AppExit>) {
    if trigger.event().name != "quit" {
        return;
    }
    exit.write(AppExit::Success);
}
