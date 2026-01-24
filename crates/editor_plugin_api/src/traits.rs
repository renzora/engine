//! Plugin traits that plugins must implement.

use crate::abi::{PluginError, PluginManifest};
use crate::api::EditorApi;
use crate::events::EditorEvent;

// Re-export bevy types for plugins
pub use bevy::app::App;
pub use bevy::ecs::world::World;
pub use bevy::ecs::system::Commands;

/// Main plugin trait - all plugins must implement this.
///
/// This trait defines the lifecycle and interface for editor plugins.
/// Plugins can extend the editor with new functionality like script engines,
/// custom gizmos, inspectors, and panels.
///
/// # Important: FFI Safety
///
/// Plugins are loaded as dynamic libraries (DLLs). The `declare_plugin!` macro
/// handles FFI-safe exports automatically. Do not try to export trait objects
/// directly across DLL boundaries.
///
/// # Example
///
/// ```rust,ignore
/// use editor_plugin_api::prelude::*;
///
/// pub struct MyPlugin {
///     counter: i32,
/// }
///
/// impl EditorPlugin for MyPlugin {
///     fn manifest(&self) -> PluginManifest {
///         PluginManifest::new("com.example.my-plugin", "My Plugin", "1.0.0")
///     }
///
///     fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
///         api.log_info("Plugin loaded!");
///         Ok(())
///     }
///
///     fn on_unload(&mut self, _api: &mut dyn EditorApi) {}
///     fn on_update(&mut self, _api: &mut dyn EditorApi, _dt: f32) {}
///     fn on_event(&mut self, _api: &mut dyn EditorApi, _event: &EditorEvent) {}
/// }
///
/// declare_plugin!(MyPlugin, MyPlugin { counter: 0 });
/// ```
pub trait EditorPlugin: Send + Sync {
    /// Return the plugin manifest with metadata
    fn manifest(&self) -> PluginManifest;

    /// Called during App building to register Bevy systems and resources.
    ///
    /// Note: This method is not called through FFI for safety reasons.
    /// Use on_load and on_update for most plugin functionality.
    fn build(&self, _app: &mut App) {
        // Default: no systems to register
    }

    /// Called when the plugin is loaded.
    /// Use this to register UI elements like panels, menus, status bar items.
    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError>;

    /// Called when the plugin is about to be unloaded.
    /// Clean up any resources here.
    fn on_unload(&mut self, api: &mut dyn EditorApi);

    /// Called every frame for UI updates.
    /// Use this for updating panels, status bar, etc.
    fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32);

    /// Called every frame with direct World access.
    ///
    /// Note: This method is not called through FFI for safety reasons.
    /// Direct World access from plugins requires careful ABI compatibility.
    fn on_world_update(&mut self, _world: &mut World) {
        // Default: no world operations
    }

    /// Called when an editor event occurs.
    /// Only receives events the plugin subscribed to.
    fn on_event(&mut self, api: &mut dyn EditorApi, event: &EditorEvent);
}
