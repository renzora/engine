//! `script!` and the entry points it emits.
//!
//! Moved here from `renzora`'s `plugin_meta` with the types it names
//! (`ScriptCtx`, `ScriptHook`), so the macro and what it expands to live in one
//! crate. Scripts keep writing `renzora::script!`: `renzora_sdk` re-exports it.

/// Declare a **Rust script**: per-entity native code, compiled from the
/// project's `scripts/` directory.
///
/// ```ignore
/// use bevy::prelude::*;
/// use renzora::ScriptCtx;
///
/// fn update(ctx: &mut ScriptCtx) {
///     let dt = ctx.delta();
///     if let Some(mut t) = ctx.get_mut::<Transform>() {
///         t.rotate_y(dt);
///     }
/// }
///
/// renzora::script!(update);
/// ```
///
/// Attach it by dropping the file into an entity's **Scripts** component, the
/// same way a `.lua` script attaches: routing is by file extension.
///
/// [`ScriptCtx`](crate::ScriptCtx) is the script's own entity plus the world:
/// `get`/`get_mut`/`insert` act on itself with no argument, and `ctx.world()`
/// hands back the whole `&mut World`. Nothing is withheld: spawning
/// hierarchies, building UI, querying everything, swapping assets are all one
/// call away.
///
/// A script is a native plugin (`renzora::plugin!`) with a per-entity convention
/// on top, built by the same compiler against the same SDK. The limits are
/// therefore the plugin limits: no hot unload, and nothing in a statically
/// linked build.
///
/// Takes a path, so the function may be named anything and live anywhere in the
/// file: `renzora::script!(behaviour::update)` is fine.
///
/// # Lifecycle hooks
///
/// A second, optional entry point receives everything that is not the per-frame
/// update: the Rust counterpart to Lua's `on_ready`, `on_ui`, `on_rpc`,
/// `on_scene_loaded` and the rest:
///
/// ```ignore
/// fn update(ctx: &mut ScriptCtx) { /* every frame */ }
///
/// fn hooks(ctx: &mut ScriptCtx, hook: &renzora::ScriptHook) {
///     match hook {
///         renzora::ScriptHook::Ready => { /* once, before the first update */ }
///         renzora::ScriptHook::SceneLoaded { path, .. } => { /* a scene arrived */ }
///         _ => {}
///     }
/// }
///
/// renzora::script!(update, hooks = hooks);
/// ```
///
/// See [`ScriptHook`](crate::ScriptHook) for the event list and why one function
/// takes them all rather than eight named exports.
#[macro_export]
macro_rules! script {
    ($f:path) => {
        $crate::__script_entry!($f);
    };
    ($f:path, hooks = $h:path) => {
        $crate::__script_entry!($f);
        $crate::__script_hook_entry!($h);
    };
}

/// The entry point [`script!`] emits, with the export attribute the current link
/// mode needs.
///
/// Split out because a `#[cfg(feature = ...)]` written inside `script!`'s
/// expansion would be evaluated when the **script** is compiled, against the
/// script's own manifest, where `static_scripts` does not exist and never will.
/// Defining the two variants here evaluates the cfg where the feature actually
/// lives: on this crate, which cargo compiles once per build and whose features
/// are unified across it.
#[doc(hidden)]
#[cfg(not(feature = "static_scripts"))]
#[macro_export]
macro_rules! __script_entry {
    ($f:path) => {
        /// The one symbol the dispatcher looks up. See [`script!`].
        ///
        /// The context is built here rather than by the dispatcher so the
        /// boundary stays one plain function pointer: nothing with a lifetime
        /// crosses it.
        #[unsafe(no_mangle)]
        pub fn renzora_script_update(
            world: &mut $crate::bevy::ecs::world::World,
            entity: $crate::bevy::ecs::entity::Entity,
        ) {
            let mut ctx = $crate::ScriptCtx::new(world, entity);
            $f(&mut ctx)
        }
    };
}

/// The optional hook entry point [`script!`]'s two-argument form emits.
///
/// Separate from [`__script_entry!`] and separately optional, so the dispatcher
/// can tell "this script has no hooks" from "this script has hooks that ignore
/// everything" by whether the symbol resolves, and so a script written before
/// hooks existed keeps working untouched.
#[doc(hidden)]
#[cfg(not(feature = "static_scripts"))]
#[macro_export]
macro_rules! __script_hook_entry {
    ($h:path) => {
        #[unsafe(no_mangle)]
        pub fn renzora_script_hook(
            world: &mut $crate::bevy::ecs::world::World,
            entity: $crate::bevy::ecs::entity::Entity,
            hook: &$crate::ScriptHook<'_>,
        ) {
            let mut ctx = $crate::ScriptCtx::new(world, entity);
            $h(&mut ctx, hook)
        }
    };
}

/// Linked-in variant of the hook entry, for the same reason as
/// [`__script_entry!`]'s: fifty scripts in one binary cannot each export the
/// symbol.
#[doc(hidden)]
#[cfg(feature = "static_scripts")]
#[macro_export]
macro_rules! __script_hook_entry {
    ($h:path) => {
        pub fn renzora_script_hook(
            world: &mut $crate::bevy::ecs::world::World,
            entity: $crate::bevy::ecs::entity::Entity,
            hook: &$crate::ScriptHook<'_>,
        ) {
            let mut ctx = $crate::ScriptCtx::new(world, entity);
            $h(&mut ctx, hook)
        }
    };
}

/// Linked-in variant: no `#[no_mangle]`, so every script in a project can be
/// compiled into one binary.
///
/// Without this, fifty scripts would define fifty `renzora_script_update`
/// symbols and the exported game would not link. Each script is a module of the
/// generated aggregator crate, so the un-mangled functions do not collide: the
/// aggregator names them by path (`script_0::renzora_script_update`) instead of
/// looking them up by symbol.
#[doc(hidden)]
#[cfg(feature = "static_scripts")]
#[macro_export]
macro_rules! __script_entry {
    ($f:path) => {
        pub fn renzora_script_update(
            world: &mut $crate::bevy::ecs::world::World,
            entity: $crate::bevy::ecs::entity::Entity,
        ) {
            let mut ctx = $crate::ScriptCtx::new(world, entity);
            $f(&mut ctx)
        }
    };
}
