mod backend;
mod command;
mod component;
mod context;
mod engine;
pub mod extension;
pub mod get_handler;
pub mod http;
pub mod plugin_backend;
pub mod plugin_bridge;
mod input;
mod plugin;

pub mod api;
pub mod perf;
pub mod resources;
pub mod systems;

#[cfg(test)]
pub(crate) mod test_util;

pub use backend::*;
pub use command::*;
pub use component::*;
pub use context::*;
pub use engine::*;
pub use extension::*;
pub use get_handler::{
    AssetProgressBridge, AssetProgressSnapshot, SceneLoadBridge, SceneLoadSnapshot,
};
pub use input::*;
pub use plugin::*;

/// Starter contents for a new `.lua` script.
///
/// Lives here, beside the hook vocabulary it demonstrates, so the two places
/// that create scripts — the Assets panel's New menu and the hierarchy's
/// right-click Attach — write the same file. The same reason
/// `renzora_blueprint::starter_blueprint_json` lives in the blueprint crate.
///
/// `boilerplate` off gives a bare comment: Lua needs no skeleton to be a valid
/// script, so "minimal" really is almost empty here — unlike Rust, which needs
/// its entry-point macro either way.
pub fn starter_lua(boilerplate: bool) -> String {
    if !boilerplate {
        return "-- New Lua script\n".to_string();
    }
    // Both hooks, and the one thing about transforms that catches everyone: the
    // `position_*` globals are read-only inputs, so moving an entity means
    // calling a function.
    concat!(
        "-- Attached to an entity. The engine calls these hooks; delete the\n",
        "-- ones you don't need.\n",
        "\n",
        "function on_ready()\n",
        "    -- Once, when the entity's scripts start.\n",
        "end\n",
        "\n",
        "function on_update()\n",
        "    -- Every frame. `delta` is seconds since the last one.\n",
        "    --\n",
        "    -- `position_x`, `rotation_y`, … are read-only inputs refreshed each\n",
        "    -- frame — assigning to them does nothing. Move an entity by calling\n",
        "    -- translate() / set_position(), or through physics.\n",
        "    local speed = 5.0\n",
        "    translate(input_x * speed * delta, 0, input_y * speed * delta)\n",
        "end\n",
    )
    .to_string()
}

/// Starter contents for a new `.rs` script.
///
/// `boilerplate` only decides whether the body is commented and illustrative.
/// The `use` lines and `renzora::script!` are written either way: a `.rs`
/// without that macro compiles, loads, and then reports "exports no entry
/// point", which is a poor first impression of a feature whose whole promise is
/// that it compiles.
pub fn starter_rust(boilerplate: bool) -> String {
    if !boilerplate {
        return concat!(
            "use bevy::prelude::*;\n",
            "use renzora::ScriptCtx;\n",
            "\n",
            "fn update(ctx: &mut ScriptCtx) {\n",
            "    let _ = ctx;\n",
            "}\n",
            "\n",
            "renzora::script!(update);\n",
        )
        .to_string();
    }
    concat!(
        "// A Rust script. Compiled to a native plugin on save and called once\n",
        "// per frame for each entity it is attached to, with full `&mut World`\n",
        "// access — which is the reason to write one instead of Lua.\n",
        "use bevy::prelude::*;\n",
        "use renzora::ScriptCtx;\n",
        "\n",
        "fn update(ctx: &mut ScriptCtx) {\n",
        "    let dt = ctx.delta();\n",
        "    if let Some(mut transform) = ctx.get_mut::<Transform>() {\n",
        "        transform.rotate_y(dt);\n",
        "    }\n",
        "}\n",
        "\n",
        "// Exports the entry point. Without it the script builds and then loads\n",
        "// as \"exports no entry point\".\n",
        "renzora::script!(update);\n",
    )
    .to_string()
}
