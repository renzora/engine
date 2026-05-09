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
