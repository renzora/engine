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
