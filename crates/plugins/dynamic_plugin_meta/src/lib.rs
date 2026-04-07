//! Macros and types for Renzora dynamic plugins.
//!
//! Plugin authors implement `Plugin` for their type and call `add!(MyPlugin)`.
//! The macro generates the FFI export. The plugin code is standard Bevy.

pub use bevy;

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
/// add!(MyPlugin);
///
/// // Editor only — won't ship with exported games
/// add!(MyEditorTool, Editor);
///
/// // Runtime only — gameplay systems, no editor UI
/// add!(MyGameplay, Runtime);
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

        /// Returns the TypeId of bevy::ecs::world::World as two u64s.
        /// The engine compares this against its own TypeId to verify compatibility.
        #[no_mangle]
        pub extern "C" fn plugin_bevy_hash() -> [u64; 2] {
            let id = std::any::TypeId::of::<$crate::bevy::ecs::world::World>();
            unsafe { std::mem::transmute(id) }
        }
    };
}
