//! Dynamic plugin macro + scope enum.
//!
//! Every Renzora plugin uses [`add!`] to expose itself across the FFI
//! boundary so the editor (or runtime) can load it from a `.dll` / `.so` /
//! `.dylib` at startup. Originally lived in a separate `dynamic_plugin_meta`
//! crate; consolidated here so plugin authors only need `bevy` + `renzora`
//! as dependencies.

/// Plugin scope — determines when the plugin is loaded.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginScope {
    Editor = 0,
    Runtime = 1,
    EditorAndRuntime = 2,
}

/// Export a Bevy plugin from a dynamic library.
///
/// # Examples
///
/// ```rust,ignore
/// // Loads in both editor and exported games (default)
/// renzora::add!(MyPlugin);
///
/// // Editor only — won't ship with exported games
/// renzora::add!(MyEditorTool, Editor);
///
/// // Runtime only — gameplay systems, no editor UI
/// renzora::add!(MyGameplay, Runtime);
/// ```
#[macro_export]
macro_rules! add {
    ($plugin_type:ty) => {
        $crate::add!($plugin_type, EditorAndRuntime);
    };
    ($plugin_type:ty, $scope:ident) => {
        #[no_mangle]
        pub extern "C" fn plugin_create() -> *mut dyn $crate::bevy::app::Plugin {
            Box::into_raw(Box::new(<$plugin_type>::default()) as Box<dyn $crate::bevy::app::Plugin>)
        }

        #[no_mangle]
        pub extern "C" fn plugin_scope() -> u8 {
            $crate::PluginScope::$scope as u8
        }

        #[no_mangle]
        pub extern "C" fn plugin_bevy_hash() -> [u64; 2] {
            let id = std::any::TypeId::of::<$crate::bevy::ecs::world::World>();
            unsafe { std::mem::transmute(id) }
        }
    };
}
