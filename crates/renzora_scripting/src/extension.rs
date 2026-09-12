//! Script functions a domain crate *declares* rather than writes.
//!
//! The whole vocabulary moved to [`renzora::script_extension`], and this module
//! re-exports it. It had to move: a plugin reaches `bevy`, `renzora` and
//! `renzora_ember` and nothing else, so while these types lived here the one
//! mechanism for extending the script API was open only to crates compiled into
//! the engine.
//!
//! The re-export is not a deprecation shim. `renzora_scripting` is where a
//! reader looks for anything about scripting, and `use
//! renzora_scripting::extension::Bind` is what ~25 call sites across the engine
//! already say; pointing them at a second path would buy nothing.

pub use renzora::script_extension::{
    substitute, Bind, Binding, BindingKind, Param, ParamKind, ScriptExtension, ScriptExtensions,
};
