//! Plugin declaration.
//!
//! Every Renzora plugin declares itself with [`add!`] in its own crate:
//!
//! ```rust,ignore
//! renzora::add!(MyPlugin);                    // Runtime (the default)
//! renzora::add!(MyEditorTool, Editor);        // Editor-only
//! renzora::add!(MyFoundation, Runtime, priority = -100);
//! ```
//!
//! **The macro emits no registration code.** It expands to a compile-time check
//! that the named type really is a `Plugin` with a `Default` impl, and that is
//! all it does at compile time. What makes the plugin *run* is the wiring
//! generated from these declarations: `cargo renzora sync` scans every crate
//! under `crates/` for `add!` lines and regenerates
//!
//! - the `[dependencies]` entry that links the crate into the binary, and
//! - `plugins.rs` in `renzora_runtime` (Runtime scope) or `renzora_editor`
//!   (Editor scope), which is an ordinary list of `app.add_plugins(...)` calls.
//!
//! So dropping a crate into `crates/` with an `add!` line in it is the whole
//! job — the generator finds it, links it and installs it. Both generated files
//! are committed, so a plain `cargo build` needs no generator run; CI checks
//! that regenerating produces no diff, which is what stops a stale list from
//! shipping.
//!
//! ## Why this isn't a registry any more
//!
//! It used to be. `add!` submitted an `inventory` entry and the host iterated
//! the registry at startup, because plugins were `dlopen`'d and a plugin the
//! host had never heard of had to be able to announce itself. Nothing is
//! `dlopen`'d against Bevy now — the editor is a binary and third-party
//! extensions are C-ABI plugins (`renzora_plugin`) that link no Bevy at all —
//! so there was no longer anyone to announce *to*. Deleting the registry also
//! deleted the three dead-strip workarounds that existed only to keep its
//! constructors alive: the keepalive `build.rs` in `renzora_runtime` and
//! `renzora_editor`, and the `renzora_static_plugins` aggregator that forced
//! plugin objects into a lean export. A named type in a generated list needs
//! none of that; the linker can see it.
//!
//! ## What the generator reads
//!
//! The declaration is parsed as text, so keep it on one line and at the top
//! level of the file (a commented-out or string-embedded `add!` is ignored
//! because the parse requires the full `add!(..);` form at line start). The
//! plugin type is resolved from the module the file defines — a declaration in
//! `src/material/mod.rs` becomes `mycrate::material::MyPlugin` — so every module
//! on that path must be `pub`. A wrong path is a compile error in the generated
//! file, never a silently missing plugin.
//!
//! `priority` orders the generated list (lower = installed earlier, default 0).
//! Reach for it only when a plugin must initialize before another; ordering
//! between systems belongs in Bevy's own system sets.

/// Declare a Bevy plugin so the build wires it into the engine.
///
/// See the [module docs](self) for what this does and does not do. The plugin
/// type must implement [`Default`]; if it needs a non-default constructor,
/// implement `Default` to delegate to it.
///
/// There is no "both" scope: a plugin is exclusively `Runtime` or `Editor`. A
/// feature needing editor tooling on top of runtime behaviour ships two plugins
/// (e.g. `GameUiPlugin` + `GameUiEditorPlugin`).
#[macro_export]
macro_rules! add {
    ($plugin_type:ty) => {
        $crate::add!($plugin_type, Runtime, priority = 0);
    };
    ($plugin_type:ty, $scope:ident) => {
        $crate::add!($plugin_type, $scope, priority = 0);
    };
    ($plugin_type:ty, $scope:ident, priority = $priority:expr) => {
        // Type check only — the generated list is what installs the plugin.
        // Catching `Plugin`/`Default` here keeps the error in the plugin's own
        // crate, where the author can read it, instead of surfacing in a
        // generated file they didn't write.
        const _: fn() = || {
            fn assert_declarable<T: $crate::bevy::app::Plugin + ::std::default::Default>() {}
            assert_declarable::<$plugin_type>();
        };
    };
}

/// Declare a **native plugin**: a Bevy plugin shipped as Rust source and
/// compiled on the machine that installs it.
///
/// The counterpart to [`add!`]. `add!` declares a crate compiled INTO the
/// engine; this declares one installed into `<exe dir>/plugins/` and rebuilt
/// whenever the engine moves under it. Both are ordinary Bevy plugins with full
/// `&mut World` access — unlike a C-ABI plugin, which links no Bevy and reaches
/// the engine through a fixed function table. All three are "plugins" to a user;
/// the difference is only in how they are built and where they can run.
///
/// ```ignore
/// use bevy::prelude::*;
///
/// pub struct SpinThing;
/// impl Plugin for SpinThing {
///     fn build(&self, app: &mut App) { app.add_systems(Update, spin); }
/// }
///
/// renzora::plugin!(SpinThing);
/// ```
///
/// Takes an expression, not just a type, so a plugin needing configuration can
/// write `renzora::plugin!(SpinThing::new(4))` rather than contorting itself
/// into a `Default` impl.
///
/// # Why this exists rather than the four lines it expands to
///
/// The loader finds a plugin by asking the OS for one symbol *by name*. Every
/// way of writing that by hand fails identically — the library loads, the symbol
/// is absent, and it is **skipped in silence**, because a library without the
/// entry point is not an error, it is simply not a plugin:
///
/// * a typo in `renzora_native_plugin_ctor`
/// * a missing `#[unsafe(no_mangle)]`, which leaves the symbol mangled with an
///   unpredictable hash
/// * a wrong return type, which is worse than silent: the loader calls it as
///   `fn() -> Box<dyn Plugin>` regardless and reads whatever is in the return
///   register as a boxed trait object
///
/// The macro fixes the name and the signature, so none of the three is reachable.
/// # Scope
///
/// `renzora::plugin!(MyThing, Runtime)` declares a plugin that also belongs in a
/// shipped game; `Editor` (the default) keeps it to the editor.
///
/// A runtime native plugin works for the same reason the editor's does: an
/// exported game built by the copy-based modes ships the very `bevy_dylib` and
/// `renzora_dylib` the plugin was compiled against, so the `World` on both sides
/// of the boundary is one type. It is the *lean* export that cannot host one —
/// that binary links Bevy statically and shares no image, which is the same
/// reason a Rust script has to be compiled into it instead.
///
/// `Editor` is the default because that is what every native plugin written so
/// far is, and because the cost of guessing wrong points the safe way: an
/// editor-scoped plugin merely does not appear in a game, while a
/// runtime-scoped one that should not have shipped is in the player's hands.
#[macro_export]
macro_rules! plugin {
    ($plugin:expr) => {
        $crate::plugin!($plugin, Editor);
    };
    ($plugin:expr, $scope:ident) => {
        $crate::__native_plugin_entry!($plugin, $scope);
    };
}

/// The symbols [`plugin!`] emits, or nothing when the plugin is being compiled
/// INTO a binary rather than loaded from one.
///
/// Split out for the same reason [`__script_entry!`] is: a
/// `#[cfg(feature = ...)]` written inside `plugin!`'s expansion would be
/// evaluated when the **plugin** is compiled, against the plugin's own manifest,
/// where `static_plugins` does not exist and never will. Defining the two
/// variants here evaluates the cfg where the feature lives — on `renzora`, which
/// cargo compiles once per build and whose features are unified across it.
///
/// Both symbols are `#[no_mangle]`, so linking fifty plugins into one binary
/// defines each of them fifty times and the link fails. The lean exporter turns
/// this feature on and calls `add_plugins` on the plugin type directly, which
/// needs no symbol at all.
#[doc(hidden)]
#[cfg(not(feature = "static_plugins"))]
#[macro_export]
macro_rules! __native_plugin_entry {
    ($plugin:expr, $scope:ident) => {
        /// The one symbol the loader looks up. Unmangled so it can be found by
        /// string; see [`plugin!`] for why hand-writing this is a trap.
        #[unsafe(no_mangle)]
        pub fn renzora_native_plugin_ctor() -> ::std::boxed::Box<dyn $crate::bevy::app::Plugin> {
            ::std::boxed::Box::new($plugin)
        }

        /// Where this plugin may load, as a byte the loader reads by symbol.
        ///
        /// A plain `u8` rather than an enum, because this crosses a `dlopen`
        /// boundary: a `#[repr(Rust)]` enum has no guaranteed layout, and the
        /// loader would be reading whichever bytes the compiler happened to
        /// choose. The absence of the symbol reads as `Editor`, so a plugin
        /// built before this existed keeps its old behaviour rather than
        /// silently appearing in someone's game.
        #[unsafe(no_mangle)]
        pub extern "C" fn renzora_native_plugin_scope() -> u8 {
            $crate::NativePluginScope::$scope as u8
        }
    };
}

/// [`__native_plugin_entry!`] for a build that links its plugins in.
///
/// Emits nothing. The plugin's type is defined by the author's own code and is
/// all the generated aggregator needs; the scope was decided at export time, by
/// reading it from the built library rather than from this declaration.
#[doc(hidden)]
#[cfg(feature = "static_plugins")]
#[macro_export]
macro_rules! __native_plugin_entry {
    ($plugin:expr, $scope:ident) => {};
}

/// Where a native plugin is allowed to load.
///
/// The native counterpart to the C-ABI `renzora_plugin::sys::PluginScope`, kept
/// separate because the two cross different boundaries and neither should be
/// able to drift into the other's ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum NativePluginScope {
    /// Editor only. The default, and what every native plugin was before scopes
    /// existed.
    #[default]
    Editor = 0,
    /// Also loaded by a shipped game — see [`plugin!`] for what makes that sound.
    Runtime = 1,
}

impl NativePluginScope {
    /// Decode the byte the `renzora_native_plugin_scope` symbol returns.
    ///
    /// Anything unrecognised is `Editor`: a plugin built against a newer engine
    /// that grew a third scope should stay out of a game this one ships, rather
    /// than being admitted on the strength of a byte this build cannot read.
    pub fn from_byte(b: u8) -> Self {
        match b {
            1 => Self::Runtime,
            _ => Self::Editor,
        }
    }
}

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
/// same way a `.lua` script attaches — routing is by file extension.
///
/// [`ScriptCtx`](crate::ScriptCtx) is the script's own entity plus the world:
/// `get`/`get_mut`/`insert` act on itself with no argument, and `ctx.world()`
/// hands back the whole `&mut World`. Nothing is withheld — spawning
/// hierarchies, building UI, querying everything, swapping assets are all one
/// call away.
///
/// A script is a [native plugin](plugin!) with a per-entity convention on top,
/// built by the same compiler against the same SDK. The limits are therefore the
/// plugin limits: no hot unload, and nothing in a statically linked build.
///
/// Takes a path, so the function may be named anything and live anywhere in the
/// file — `renzora::script!(behaviour::update)` is fine.
/// # Lifecycle hooks
///
/// A second, optional entry point receives everything that is not the per-frame
/// update — the Rust counterpart to Lua's `on_ready`, `on_ui`, `on_rpc`,
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
/// Split out for the same reason `renzora_plugin`'s `__plugin_scope_entry!` is:
/// a `#[cfg(feature = ...)]` written inside `script!`'s expansion would be
/// evaluated when the **script** is compiled, against the script's own manifest,
/// where `static_scripts` does not exist and never will. Defining the two
/// variants here evaluates the cfg where the feature actually lives — on
/// `renzora`, which cargo compiles once per build and whose features are unified
/// across it.
#[doc(hidden)]
#[cfg(not(feature = "static_scripts"))]
#[macro_export]
macro_rules! __script_entry {
    ($f:path) => {
        /// The one symbol the dispatcher looks up. See [`script!`].
        ///
        /// The context is built here rather than by the dispatcher so the
        /// boundary stays one plain function pointer — nothing with a lifetime
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
/// everything" by whether the symbol resolves — and so a script written before
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
/// generated aggregator crate, so the un-mangled functions do not collide — the
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
