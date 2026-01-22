//! Plugin traits that plugins must implement.

use super::abi::{PluginError, PluginManifest};
use super::api::EditorApi;
use crate::ui_api::UiEvent;

/// Editor events that plugins can receive
#[derive(Clone, Debug)]
pub enum EditorEvent {
    /// Entity was selected
    EntitySelected(super::abi::EntityId),
    /// Entity was deselected
    EntityDeselected(super::abi::EntityId),
    /// Scene was loaded
    SceneLoaded { path: String },
    /// Scene was saved
    SceneSaved { path: String },
    /// Play mode started
    PlayStarted,
    /// Play mode stopped
    PlayStopped,
    /// Project was opened
    ProjectOpened { path: String },
    /// Project was closed
    ProjectClosed,
    /// UI event from a plugin-registered widget
    UiEvent(UiEvent),
    /// Custom event from another plugin
    CustomEvent { plugin_id: String, event_type: String, data: Vec<u8> },
}

/// Event types for subscription
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditorEventType {
    EntitySelected,
    EntityDeselected,
    SceneLoaded,
    SceneSaved,
    PlayStarted,
    PlayStopped,
    ProjectOpened,
    ProjectClosed,
    UiEvent,
    CustomEvent,
    All,
}

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

/// Type alias for the plugin creation function
pub type CreatePluginFn = unsafe extern "C" fn() -> *mut dyn EditorPlugin;

/// Macro for plugin authors to declare their plugin entry point
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $constructor:expr) => {
        #[no_mangle]
        pub extern "C" fn create_plugin() -> *mut dyn $crate::plugin_core::EditorPlugin {
            let plugin: Box<dyn $crate::plugin_core::EditorPlugin> = Box::new($constructor);
            Box::into_raw(plugin)
        }
    };
}

/// Internal: wraps a dynamically loaded plugin
pub(crate) struct LoadedPluginWrapper {
    plugin: Box<dyn EditorPlugin>,
    manifest: PluginManifest,
    enabled: bool,
    subscriptions: Vec<EditorEventType>,
}

impl LoadedPluginWrapper {
    pub fn new(plugin: Box<dyn EditorPlugin>) -> Self {
        let manifest = plugin.manifest();
        Self {
            plugin,
            manifest,
            enabled: true,
            subscriptions: Vec::new(),
        }
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn subscribe(&mut self, event_type: EditorEventType) {
        if !self.subscriptions.contains(&event_type) {
            self.subscriptions.push(event_type);
        }
    }

    pub fn is_subscribed(&self, event_type: EditorEventType) -> bool {
        self.subscriptions.contains(&event_type) || self.subscriptions.contains(&EditorEventType::All)
    }

    pub fn plugin_mut(&mut self) -> &mut dyn EditorPlugin {
        &mut *self.plugin
    }
}
