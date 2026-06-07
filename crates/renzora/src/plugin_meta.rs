//! Unified plugin registration.
//!
//! Every Renzora plugin uses [`add!`] to declare itself. The macro emits
//! two registration paths in parallel; the runtime picks whichever fits
//! the build target:
//!
//! 1. **Inventory** — a `StaticPlugin` entry in the runtime registry
//!    populated by `inventory::submit!`. When the host iterates the
//!    registry at startup, every plugin compiled into the binary (or
//!    dlopen'd from a distribution dylib in `plugins/`) self-registers,
//!    no manual enumeration. Used for engine-internal plugins on every
//!    platform: desktop binary, iOS staticlib, Android cdylib, wasm32.
//!
//! 2. **FFI** — `extern "C"` exports (`plugin_create`, `plugin_scope`,
//!    `plugin_bevy_hash`) so the dynamic plugin loader can dlopen a
//!    standalone `.dll` / `.so` / `.dylib` and read the symbols. Used for
//!    user-installed plugins on desktop. Multiple statically-linked plugins
//!    would have these symbols collide, so they're cfg-gated to desktop
//!    targets only.
//!
//! The same `renzora::add!(MyPlugin)` line works for both. Plugin authors
//! don't think about static-vs-dynamic; the macro and the runtime handle it.

use bevy::app::App;

/// Plugin scope — determines when the plugin is loaded.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginScope {
    Editor = 0,
    Runtime = 1,
    EditorAndRuntime = 2,
}

impl PluginScope {
    /// Whether a plugin with `self` scope should be loaded for a host
    /// running in `host` scope.
    ///
    /// `EditorAndRuntime` plugins are loaded as part of the Runtime pass,
    /// not the Editor pass. The editor build calls both
    /// `add_engine_plugins(Runtime)` and `add_editor_plugins(Editor)`,
    /// so if `EditorAndRuntime` matched both we'd register the same
    /// plugin twice (Bevy panics with "plugin was already added"). Treat
    /// the Runtime pass as the canonical home for "loads everywhere"
    /// plugins; Editor only adds the editor-only ones on top.
    #[inline]
    pub fn matches(self, host: PluginScope) -> bool {
        match (self, host) {
            (PluginScope::EditorAndRuntime, PluginScope::Runtime) => true,
            (a, b) => a == b,
        }
    }
}

/// One entry in the plugin registry. Each `renzora::add!(...)` invocation
/// produces one of these and submits it via `inventory::submit!`.
pub struct StaticPlugin {
    /// Human-readable name (the plugin type's `stringify!`'d name).
    /// Useful for debug logging when a plugin fails to register.
    pub name: &'static str,

    /// When this plugin should load.
    pub scope: PluginScope,

    /// Order hint. Lower = registered earlier. Default 0; reach for
    /// non-zero values only when a plugin must initialize before another.
    /// Most plugins should rely on Bevy's own system-set ordering instead.
    pub priority: i32,

    /// Installer. Constructs the plugin and adds it to the App. Called once
    /// at startup if [`scope`](Self::scope) matches the host scope. The
    /// indirection through a function pointer (rather than `Box<dyn Plugin>`)
    /// lets us call `app.add_plugins(...)` directly — Bevy's `Plugins`
    /// trait isn't implemented for trait objects.
    pub install: fn(&mut App),
}

// Tell `inventory` that `StaticPlugin` is a collectable type. Each
// `inventory::submit!` block (emitted by `add!` below) registers one
// entry. The collection is populated at process startup by ctor
// functions, plus on dlopen for any distribution plugin loaded later.
inventory::collect!(StaticPlugin);

/// Iterate every registered plugin, filtering by `host_scope` and
/// invoking `f` on each matching entry in priority order.
///
/// The runtime calls this once at startup. Plugins with mismatched scope
/// (e.g. `Editor` plugin during a `Runtime` build) are skipped.
pub fn for_each_static_plugin<F: FnMut(&'static StaticPlugin)>(host_scope: PluginScope, mut f: F) {
    let mut entries: Vec<&'static StaticPlugin> = inventory::iter::<StaticPlugin>
        .into_iter()
        .filter(|p| p.scope.matches(host_scope))
        .collect();
    entries.sort_by_key(|p| p.priority);
    for entry in entries {
        f(entry);
    }
}

/// Register a Bevy plugin with the Renzora engine.
///
/// # Examples
///
/// ```rust,ignore
/// // Loads in both editor and exported games (default scope).
/// renzora::add!(MyPlugin);
///
/// // Editor only — won't ship with exported games.
/// renzora::add!(MyEditorTool, Editor);
///
/// // Runtime only — gameplay systems, no editor UI.
/// renzora::add!(MyGameplay, Runtime);
///
/// // With explicit priority (lower = earlier; default 0).
/// renzora::add!(MyFoundation, EditorAndRuntime, priority = -100);
/// ```
///
/// The plugin type must implement [`Default`]. If your plugin needs a
/// non-default constructor, implement `Default` to delegate to it.
#[macro_export]
macro_rules! add {
    ($plugin_type:ty) => {
        $crate::add!($plugin_type, EditorAndRuntime, priority = 0);
    };
    ($plugin_type:ty, $scope:ident) => {
        $crate::add!($plugin_type, $scope, priority = 0);
    };
    ($plugin_type:ty, $scope:ident, priority = $priority:expr) => {
        // Runtime registration via `inventory`. Each call expands to a
        // ctor function (uniquely named per-call so multiple `add!`
        // invocations in one module don't collide) that pushes a
        // `StaticPlugin` entry into the shared registry. Works on every
        // platform: desktop binary, mobile staticlib/cdylib, wasm32.
        $crate::inventory::submit! {
            $crate::StaticPlugin {
                name: stringify!($plugin_type),
                scope: $crate::PluginScope::$scope,
                priority: $priority,
                install: |app| {
                    app.add_plugins(<$plugin_type as ::std::default::Default>::default());
                },
            }
        }

        // FFI exports: discovered via dlopen on desktop. Gated behind a
        // `dlopen` feature *on the calling crate* (not on `renzora`),
        // because `cfg(feature = ...)` in a macro body resolves against
        // the expansion site's crate. Distribution plugins (cdylib)
        // declare their own `dlopen = []` feature and turn it on by
        // default; workspace plugins (rlib) don't, so the symbols stay
        // off and don't collide at link time inside the host binary.
        // Symbols are unmangled, so each cdylib must have exactly ONE
        // `add!` invocation. Engine crates with multiple plugins (e.g.
        // `renzora_shader::{ShaderPlugin, MaterialPlugin}`) live in the
        // host as rlibs and never enable the feature.
        #[cfg(all(
            feature = "dlopen",
            not(any(target_os = "ios", target_os = "android", target_arch = "wasm32",))
        ))]
        const _: () = {
            #[no_mangle]
            pub extern "C" fn plugin_create() -> *mut dyn $crate::bevy::app::Plugin {
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                    <$plugin_type as ::std::default::Default>::default(),
                )
                    as ::std::boxed::Box<dyn $crate::bevy::app::Plugin>)
            }

            #[no_mangle]
            pub extern "C" fn plugin_scope() -> u8 {
                $crate::PluginScope::$scope as u8
            }

            #[no_mangle]
            pub extern "C" fn plugin_bevy_hash() -> [u64; 2] {
                let id = ::std::any::TypeId::of::<$crate::bevy::ecs::world::World>();
                unsafe { ::std::mem::transmute(id) }
            }
        };
    };
}

/// Export a **bundle** of plugins from a single distribution cdylib.
///
/// A normal plugin cdylib uses [`add!`], which emits the unmangled
/// `plugin_create` / `plugin_scope` / `plugin_bevy_hash` symbol trio. Because
/// those symbols are `#[no_mangle]`, exactly ONE `add!` can be the FFI entry
/// point of a cdylib — N statically-linked plugins would collide on those names.
///
/// A bundle cdylib instead links many plugin crates as **rlibs with `dlopen`
/// OFF** (so none of them emit the colliding trio — only their collision-safe
/// `inventory::submit!` ctors run), and calls `renzora::export_plugin_bundle!()`
/// exactly ONCE. That emits a single collision-free entry point,
/// `plugin_install_scope`. The dynamic plugin loader prefers it when present and
/// falls back to `plugin_create` for single-plugin community cdylibs; both are
/// still gated on `plugin_bevy_hash`, so the ABI guard is identical.
///
/// ## The inventory is GLOBAL — read this before relying on a bundle
///
/// `inventory::collect!(StaticPlugin)` lives in the **shared `renzora` dylib**
/// (one global registry across the dylib boundary, by design). So
/// `plugin_install_scope` does NOT replay "only this bundle's linked plugins" —
/// it replays **every matching-scope plugin currently in that one global
/// registry** (which, at dlopen time, is the host's submissions PLUS this
/// bundle's rlibs' submissions). Consequences (load-bearing, not optional):
///
/// - **Deployment contract:** a build either statically links the editor plugins
///   (and installs them via `add_editor_plugins`) **OR** ships them as this
///   bundle — *never both*. If both, the editor plugins are installed twice and
///   Bevy panics "plugin was already added". The runtime-shaped host (Step C of
///   Operation Merge) must therefore NOT statically register editor-scope plugins.
/// - **Editor host installs only `Editor` scope.** `for_each_static_plugin`
///   applies [`PluginScope::matches`]: an editor host (`host_scope = Editor`)
///   gets `Editor`-only; `EditorAndRuntime`/default-scope plugins match a
///   *Runtime* host and belong in the statically-linked host, not a bundle.
/// - **Foundation plugins aren't in the inventory.** The editor SDK foundation
///   (`AssetRegistryPlugin`, `RenzoraEditorPlugin`, `KeybindingsPlugin`, …) is
///   added explicitly + ordered by `add_editor_plugins`, NOT via `add!`. Pass
///   them to the `foundation = [...]` form so they install first, in order,
///   before the inventory fan-out (later plugins read resources they init).
///
/// `plugin_install_scope` returns the number of plugins that **panicked** during
/// install (0 = all good); each install is caught individually so one bad plugin
/// can't abort the rest, and no panic ever unwinds across the `extern "C"` frame.
///
/// Gated on the **expansion-site** crate's `dlopen` feature + desktop targets,
/// same as [`add!`]'s FFI block (so a bundle crate declares its own
/// `dlopen = []` feature and turns it on by default).
///
/// ```rust,ignore
/// // No foundation (replays the global inventory's matching-scope plugins only):
/// renzora::export_plugin_bundle!();
/// // With an ordered foundation prefix installed before the fan-out:
/// renzora::export_plugin_bundle!(foundation = [
///     renzora_asset_registry::AssetRegistryPlugin,
///     renzora_editor::RenzoraEditorPlugin,
///     renzora_keybindings::KeybindingsPlugin,
/// ]);
/// ```
#[macro_export]
macro_rules! export_plugin_bundle {
    () => {
        $crate::export_plugin_bundle!(foundation = []);
    };
    (foundation = [$($foundation:path),* $(,)?]) => {
        #[cfg(all(
            feature = "dlopen",
            not(any(target_os = "ios", target_os = "android", target_arch = "wasm32",))
        ))]
        const _: () = {
            /// Install this bundle's plugins into `*app` for the given host scope
            /// and return the count of plugins that **panicked** during install
            /// (0 = all good). `host_scope` is the `PluginScope` discriminant as
            /// `u8` (Editor=0, Runtime=1, EditorAndRuntime=2). See the macro docs
            /// for the global-inventory + deployment-contract semantics.
            ///
            /// SAFETY: `app` must be a valid, exclusively-borrowed pointer to an
            /// `App` from a host compiled against the same `bevy_dylib`
            /// (guaranteed by the loader's `plugin_bevy_hash` check before call).
            #[no_mangle]
            #[allow(improper_ctypes_definitions)]
            pub extern "C" fn plugin_install_scope(
                app: *mut $crate::bevy::app::App,
                host_scope: u8,
            ) -> u32 {
                let scope = match host_scope {
                    0 => $crate::PluginScope::Editor,
                    1 => $crate::PluginScope::Runtime,
                    _ => $crate::PluginScope::EditorAndRuntime,
                };
                let app: &mut $crate::bevy::app::App = match unsafe { app.as_mut() } {
                    ::std::option::Option::Some(a) => a,
                    ::std::option::Option::None => return 1,
                };
                let mut failures: u32 = 0;
                // Foundation plugins first, in declared order — they init the
                // shared resources/registries later plugins read in `build()`.
                $(
                    if ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                        app.add_plugins(<$foundation as ::std::default::Default>::default());
                    }))
                    .is_err()
                    {
                        failures += 1;
                    }
                )*
                // Then the scope-matched plugins from the shared GLOBAL registry,
                // each caught so one panic can't abort the rest and nothing
                // unwinds across `extern "C"`.
                $crate::for_each_static_plugin(scope, |plugin| {
                    if ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                        (plugin.install)(app);
                    }))
                    .is_err()
                    {
                        failures += 1;
                    }
                });
                failures
            }

            /// Identical to [`add!`]'s `plugin_bevy_hash`: the ABI guard. The
            /// loader rejects the whole bundle if this doesn't match the host's
            /// `bevy_dylib` hash.
            #[no_mangle]
            pub extern "C" fn plugin_bevy_hash() -> [u64; 2] {
                let id = ::std::any::TypeId::of::<$crate::bevy::ecs::world::World>();
                unsafe { ::std::mem::transmute(id) }
            }
        };
    };
}
