//! The language-backend half of scripting: what a script, a language backend
//! and the engine all have to agree on.
//!
//! `renzora_scripting` owns the scripting SYSTEM. These are the types that cross
//! its boundary, and they are contract types for the reason CLAUDE.md gives: the
//! two crates that need them are the engine and an INSTALLED PLUGIN. A language
//! backend is meant to be a marketplace plugin, and while `ScriptBackend` lived
//! in `renzora_scripting` no plugin could name it, so there could be no
//! third-party language at all.
//!
//! A layer of the contract hierarchy, above `renzora`. Plugins and scripts reach
//! it all at the paths they always used (`renzora::ScriptCtx`,
//! `renzora::script!`, `renzora::command::ScriptCommand`, ...) through
//! `renzora_sdk`. The engine's own crates depend on this crate directly.
//!
//! `get_handler` is the one that could not have been solved by staging another
//! crate. It is thread-local state that answers a script's synchronous reads
//! mid-hook, so two copies means the backend reads handlers the engine never
//! wrote, and every `get` returns nothing with no error anywhere. The shared
//! `renzora_dylib` image makes it singular, exactly as it does for the
//! translation table and the console buffer.

// `script!`'s expansion names `$crate::bevy::...`, so the crate it is defined in
// has to offer Bevy at its root, the way `renzora` does.
pub use bevy;

mod script_macro;

/// What a Rust script is handed. See [`script_ctx::ScriptCtx`].
pub mod script_ctx;
pub use script_ctx::ScriptCtx;
/// The lifecycle events a Rust script can receive. See [`script_hook::ScriptHook`].
pub mod script_hook;
pub use script_hook::ScriptHook;

// The declarative script-binding vocabulary: `ScriptExtension`, `Binding`, and
// the `Bind` builder. In the contract so an installed plugin can declare script
// functions at all. `renzora_scripting::extension` re-exports it. `Param`,
// `Binding` and `Bind` are too generic for a crate root, so callers name the
// module.
pub mod script_extension;

// The backend contract and the vocabulary it speaks. `command`, `context` and
// `component` are far too generic for a crate root, so they are named in full;
// the names a backend actually writes are lifted to the root so a plugin
// implementing a language does not have to know which module each sits in.
pub mod backend;
pub mod command;
pub mod component;
pub mod context;
pub mod get_handler;
pub use backend::{AppScriptBackendExt, FileReader, ScriptBackend, ANY_HOOK};
/// How the code editor colours a language a plugin brings.
pub mod syntax;
pub use syntax::{AppCodeSyntaxExt, CodeSyntax, CodeSyntaxes};
pub use command::ScriptCommand;
pub use component::{ScriptValue, ScriptVariableDefinition, ScriptVariables};
pub use context::{GamepadSnapshot, ScriptContext, GAMEPAD_BUTTON_NAMES};

pub mod script_bridge; // the inboxes scripting drains each frame
pub mod input_actions; // named actions + the character controller queue
pub mod input_map; // the rebindable action map a controls menu changes live
pub mod reflection; // reading and writing a component field by its path
pub use input_actions::*;
// `default_actions` stays behind its module: too generic a name for the root
// every plugin and script globs.
pub use input_map::{ActionKind, InputAction, InputBinding, InputMap, RebindError};
pub use script_bridge::*;
