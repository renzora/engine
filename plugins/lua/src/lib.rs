//! Lua scripting for Renzora.
//!
//! The engine ships a scripting *system* — hooks, a command vocabulary, the
//! queue that applies commands to the world — and no interpreter. This plugin
//! supplies one. Drop it in `plugins/` and `.lua` files run; leave it out and
//! the same engine runs a game with no scripting at all, carrying none of the
//! cost.
//!
//! That is the point of the arrangement, and it generalises: a Wren or Python
//! plugin would implement the same [`Backend`] trait and claim its own
//! extensions, and neither the engine nor this crate would need to change. The
//! domain crates' script functions — `apply_force`, `nav_set_destination`,
//! `tr` — arrive as *declarations* over the boundary, so a second language gets
//! all of them without `renzora_physics` knowing it exists.
//!
//! ## Layout
//!
//! | module | what it is |
//! |---|---|
//! | [`interp`] | the interpreter, moved out of the engine largely unchanged |
//! | [`buffers`] | thread-local command/draw buffers the Lua bindings push into |
//! | [`host`] | reaching back into the engine for synchronous reads |
//!
//! [`Backend`]: renzora_plugin::script::Backend

use renzora_plugin::prelude::*;

mod buffers;
mod host;
mod interp;

#[cfg(test)]
mod tests;

// Emits the `extern "C"` entry point and the state it needs. A macro rather
// than a generic because the entry point must be a bare function pointer with
// nowhere to carry state, so it needs a `static` — and a `static` cannot be
// generic over the backend type.
renzora_plugin::script_backend!(interp::LuaBackend);

pub struct LuaPlugin;

impl Plugin for LuaPlugin {
    fn build(&self, app: &mut App) {
        app.add_script_backend(script_backend::desc());
    }
}

renzora_plugin::add!(LuaPlugin);
