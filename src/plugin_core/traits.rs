//! Plugin traits that plugins must implement.
//!
//! Re-exports types from editor_plugin_api.

// Re-export all types from the shared crate
pub use editor_plugin_api::traits::*;
pub use editor_plugin_api::events::{EditorEvent, EditorEventType};

use super::abi::PluginManifest;

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

    #[allow(dead_code)]
    pub fn is_subscribed(&self, event_type: EditorEventType) -> bool {
        self.subscriptions.contains(&event_type) || self.subscriptions.contains(&EditorEventType::All)
    }

    pub fn plugin_mut(&mut self) -> &mut dyn EditorPlugin {
        &mut *self.plugin
    }
}
