//! Editor Plugin API
//!
//! This crate provides the API for creating plugins for the Bevy Editor.
//! Plugin authors should depend on this crate to build their plugins.
//!
//! # Example
//!
//! ```rust,ignore
//! use editor_plugin_api::prelude::*;
//!
//! pub struct MyPlugin {
//!     counter: i32,
//! }
//!
//! impl EditorPlugin for MyPlugin {
//!     fn manifest(&self) -> PluginManifest {
//!         PluginManifest::new("com.example.my-plugin", "My Plugin", "1.0.0")
//!             .author("Your Name")
//!             .description("A custom editor plugin")
//!             .capability(PluginCapability::Panel)
//!     }
//!
//!     fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
//!         api.log_info("My plugin loaded!");
//!         Ok(())
//!     }
//!
//!     fn on_unload(&mut self, _api: &mut dyn EditorApi) {
//!         // Cleanup
//!     }
//!
//!     fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
//!         // Update logic
//!     }
//!
//!     fn on_event(&mut self, _api: &mut dyn EditorApi, _event: &EditorEvent) {
//!         // Handle events
//!     }
//! }
//!
//! // Export the plugin entry point
//! declare_plugin!(MyPlugin, MyPlugin { counter: 0 });
//! ```

pub mod abi;
pub mod api;
pub mod events;
pub mod ffi;
pub mod ui;

// Traits module only available with bevy feature (for editor use)
#[cfg(feature = "bevy")]
pub mod traits;

pub mod prelude;

// Re-export bevy for editor use (only when feature enabled)
#[cfg(feature = "bevy")]
pub use bevy;

// Re-export egui-phosphor icons for plugins to use directly
pub use egui_phosphor;

// Re-export core types at crate root
pub use abi::*;
pub use api::*;
pub use events::*;
pub use ffi::*;
pub use ui::*;

#[cfg(feature = "bevy")]
pub use traits::*;

/// Macro for plugin authors to declare their plugin entry point.
///
/// This creates FFI-safe exports that work reliably across DLL boundaries.
/// The macro generates:
/// - `create_plugin()` - Creates and returns the plugin with its vtable
/// - Internal wrapper functions for each plugin method
///
/// # Example
///
/// ```rust,ignore
/// declare_plugin!(MyPlugin, MyPlugin::new());
/// ```
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $constructor:expr) => {
        unsafe extern "C" fn __plugin_manifest(handle: $crate::ffi::PluginHandle) -> $crate::ffi::FfiManifest {
            let plugin = &*(handle as *const $plugin_type);
            $crate::ffi::FfiManifest::from_manifest(plugin.manifest())
        }

        unsafe extern "C" fn __plugin_on_load(
            handle: $crate::ffi::PluginHandle,
            api: $crate::ffi::EditorApiHandle,
        ) -> $crate::ffi::FfiResult {
            let plugin = &mut *(handle as *mut $plugin_type);
            let ffi_api = $crate::ffi::FfiEditorApi::new(api);
            match plugin.on_load_ffi(&ffi_api) {
                Ok(()) => $crate::ffi::FfiResult::ok(),
                Err(e) => $crate::ffi::FfiResult::err(format!("{}", e)),
            }
        }

        unsafe extern "C" fn __plugin_on_unload(
            handle: $crate::ffi::PluginHandle,
            api: $crate::ffi::EditorApiHandle,
        ) {
            let plugin = &mut *(handle as *mut $plugin_type);
            let ffi_api = $crate::ffi::FfiEditorApi::new(api);
            plugin.on_unload_ffi(&ffi_api);
        }

        unsafe extern "C" fn __plugin_on_update(
            handle: $crate::ffi::PluginHandle,
            api: $crate::ffi::EditorApiHandle,
            dt: f32,
        ) {
            let plugin = &mut *(handle as *mut $plugin_type);
            let ffi_api = $crate::ffi::FfiEditorApi::new(api);
            plugin.on_update_ffi(&ffi_api, dt);
        }

        unsafe extern "C" fn __plugin_on_event(
            handle: $crate::ffi::PluginHandle,
            _api: $crate::ffi::EditorApiHandle,
            _event_json: *const std::ffi::c_char,
        ) {
            // Events are not yet supported via FFI
            let _ = handle;
        }

        unsafe extern "C" fn __plugin_destroy(handle: $crate::ffi::PluginHandle) {
            if !handle.is_null() {
                let _ = Box::from_raw(handle as *mut $plugin_type);
            }
        }

        #[no_mangle]
        pub extern "C" fn create_plugin() -> $crate::ffi::PluginExport {
            let plugin = Box::new($constructor);
            let handle = Box::into_raw(plugin) as $crate::ffi::PluginHandle;

            $crate::ffi::PluginExport {
                ffi_version: $crate::ffi::FFI_API_VERSION,
                handle,
                vtable: $crate::ffi::PluginVTable {
                    manifest: __plugin_manifest,
                    on_load: __plugin_on_load,
                    on_unload: __plugin_on_unload,
                    on_update: __plugin_on_update,
                    on_event: __plugin_on_event,
                    destroy: __plugin_destroy,
                },
            }
        }
    };
}
