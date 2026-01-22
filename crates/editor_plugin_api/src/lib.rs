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
//! pub struct MyPlugin;
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
//!     fn on_unload(&mut self, api: &mut dyn EditorApi) {
//!         api.log_info("My plugin unloaded!");
//!     }
//!
//!     fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32) {
//!         // Called every frame
//!     }
//!
//!     fn on_event(&mut self, api: &mut dyn EditorApi, event: &EditorEvent) {
//!         // Handle editor events
//!     }
//! }
//!
//! // Export the plugin entry point
//! declare_plugin!(MyPlugin, MyPlugin);
//! ```

pub mod prelude;

// Re-export core types
// Note: In a real implementation, these would be defined here or in a shared crate
// For now, plugin authors need to use the main editor crate's types

/// Macro for plugin authors to declare their plugin entry point.
///
/// This creates the `create_plugin` function that the editor calls to instantiate the plugin.
///
/// # Example
///
/// ```rust,ignore
/// declare_plugin!(MyPlugin, MyPlugin);
/// ```
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $constructor:expr) => {
        #[no_mangle]
        pub extern "C" fn create_plugin() -> *mut dyn EditorPlugin {
            let plugin: Box<dyn EditorPlugin> = Box::new($constructor);
            Box::into_raw(plugin)
        }
    };
}
