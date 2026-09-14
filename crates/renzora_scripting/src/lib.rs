// Moved to the contract crate, for the same reason `diagnostics::script` was
// (see below) and with more force: a language backend is supposed to be a
// marketplace plugin, and an installed plugin is compiled against the staged
// SDK, which offers `bevy`, `renzora` and `renzora_ember` and nothing else. So
// while `ScriptBackend` lived here, no plugin could implement one. The Lua
// backend had been unbuildable since the FFI it used instead was deleted.
//
// `get_handler` had to travel with them rather than stay: it is THREAD-LOCAL
// state, and a plugin linking a private copy would read handlers the engine
// never wrote. Every `get("Health.current")` would return nil with nothing
// logged. In `renzora` it is covered by `renzora_dylib`, the shared image that
// already exists to make exactly this class of static singular.
//
// Only the types moved. Everything that DOES anything with them (the engine,
// the systems, the command-apply pass) stayed. Re-exported under the old paths
// so no caller, in this crate or outside it, had to change.
pub use renzora::{backend, command, component, context, get_handler};

mod engine;
pub mod extension;
pub mod http;
mod input;
mod plugin;

pub mod api;
// Moved to the contract crate so the Scripting diagnostics panel could become a
// native plugin. Timing is still collected in `systems::execution`; only the
// types moved. Re-exported under the old path so `perf::…` still resolves.
pub use renzora::diagnostics::script as perf;
pub mod resources;
pub mod systems;

#[cfg(test)]
pub(crate) mod test_util;

pub use renzora::backend::*;
pub use renzora::command::*;
pub use renzora::component::*;
pub use renzora::context::*;
pub use engine::*;
pub use extension::*;
pub use renzora::get_handler::{
    AssetProgressBridge, AssetProgressSnapshot, SceneLoadBridge, SceneLoadSnapshot,
};
pub use input::*;
pub use plugin::*;

// `starter_lua` lived here, beside the hook vocabulary it demonstrates. It went
// with the language: the Lua plugin registers its own create-menu entry and
// supplies the starter text along with it, so an engine with no interpreter no
// longer ships the first file for one. `starter_rust` stays, because the Rust
// backend is in the workspace.

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
