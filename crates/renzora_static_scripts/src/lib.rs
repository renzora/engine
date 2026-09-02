//! The project's Rust scripts, compiled into this binary.
//!
//! **This file is generated.** The version checked into the repo returns an
//! empty list; the lean exporter overwrites it (and the manifest beside it)
//! inside `target/export-src/`, the throwaway workspace copy an export compiles.
//! Editing it here changes nothing about an export.
//!
//! ## Why scripts are compiled in rather than loaded
//!
//! In the editor a `.rs` script is built into a dylib and `dlopen`'d, which is
//! sound only because script and engine share one `bevy_dylib` — one `World`
//! type, one set of `TypeId`s. A lean export links Bevy statically and has no
//! shared image, so a script dylib would carry its own copy of Bevy and calling
//! into it would be memory corruption with no diagnostic. `RustScriptPlugin`
//! refuses to register a backend in that build, which is why an exported game
//! reports `No backend for Some("rs")`.
//!
//! Compiling the source into the binary removes the boundary rather than trying
//! to make it safe: there is no library, no symbol lookup and no ABI question,
//! because the script is part of the same compilation as everything it touches.
//!
//! ## What stays identical
//!
//! The dispatcher does not change. `renzora_rust_script` fills `LoadedScripts`
//! from [`scripts()`] instead of from loaded libraries, and everything after
//! that — one `fn(&mut World, Entity)` per entity per frame, keyed by file name,
//! wrapped in a panic guard — is the same code. A script must behave the same in
//! the editor and in an export, or the export cannot be tested by playing it.
//!
//! Every script in `scripts/` is compiled in, not only those some scene
//! references: a scene can be loaded at runtime and a `ScriptComponent` added at
//! runtime, so any "which are actually used" analysis would eventually be wrong
//! in the direction that breaks a game silently. An unused script costs bytes in
//! `.text`, never cycles — the dispatcher only ever looks up names that a live
//! entity asked for.

use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;

/// A script's entry point: the same signature the dylib path calls through.
pub type ScriptFn = fn(&mut World, Entity);

/// Every script compiled into this binary, as `(file name, entry point)`.
///
/// Keyed by file NAME rather than path because that is what the dispatcher
/// resolves against: a `ScriptComponent` entry may hold a project-relative or a
/// scripts-relative path depending on how it was added, and the leaf is the same
/// either way. Scripts live in one flat directory, so leaves are unique.
///
/// Empty in the dev tree.
pub fn scripts() -> Vec<(&'static str, ScriptFn)> {
    Vec::new()
}

/// A script's optional second entry point — the lifecycle hook written by
/// `renzora::script!(update, hooks = …)`.
pub type HookFn = fn(&mut World, Entity, &renzora::ScriptHook<'_>);

/// Every script compiled in that also exported a hook, keyed the same way.
///
/// A separate table rather than an `Option` in [`scripts`] because hooks are
/// opt-in and most scripts have none — this one is usually much shorter, and it
/// keeps the common table a plain pair.
///
/// Its absence used to be a real behaviour difference between the editor and a
/// lean export: a script's `on_scene_loaded` (a loading screen's whole reason
/// for existing) fired when run from a dylib and silently never fired once
/// compiled in, because the dylib path finds hooks by looking up a second
/// symbol and a static build has nothing to look up. The tell in a build log is
/// `warning: function 'hooks' is never used`.
///
/// Empty in the dev tree.
pub fn hooks() -> Vec<(&'static str, HookFn)> {
    Vec::new()
}
