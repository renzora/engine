//! Plugin traits that plugins must implement.

use crate::abi::{PluginError, PluginManifest};
use crate::api::EditorApi;
use crate::events::EditorEvent;

/// Main plugin trait - all plugins must implement this.
///
/// This trait defines the lifecycle and interface for editor plugins.
/// Plugins can extend the editor with new functionality like script engines,
/// custom gizmos, inspectors, and panels.
pub trait EditorPlugin: Send + Sync {
    /// Return the plugin manifest with metadata
    fn manifest(&self) -> PluginManifest;

    /// Called when the plugin is loaded.
    /// Use this to register capabilities, UI elements, etc.
    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError>;

    /// Called when the plugin is about to be unloaded.
    /// Clean up any resources here.
    fn on_unload(&mut self, api: &mut dyn EditorApi);

    /// Called every frame.
    /// Use this for continuous updates like rendering custom UI.
    fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32);

    /// Called when an editor event occurs.
    /// Only receives events the plugin subscribed to.
    fn on_event(&mut self, api: &mut dyn EditorApi, event: &EditorEvent);
}

/// Type alias for the plugin creation function.
/// This is the extern "C" function that plugins export.
pub type CreatePluginFn = unsafe extern "C" fn() -> *mut dyn EditorPlugin;
