//! Editor API implementation exposed to plugins.
//!
//! Re-exports types from editor_plugin_api and provides the host-side implementation.

#![allow(dead_code)]

pub use editor_plugin_api::api::*;
pub use editor_plugin_api::events::{EditorEventType, UiEvent};

use super::abi::{EntityId, PluginTransform};

/// Tab location for docked plugin tabs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TabLocation {
    Left,
    Right,
    Bottom,
}

/// Plugin tab definition.
#[derive(Clone, Debug)]
pub struct PluginTab {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub location: TabLocation,
}

/// Pending operations that will be applied to Bevy world.
#[derive(Clone, Debug)]
pub enum PendingOperation {
    SetSelectedEntity(Option<EntityId>),
    SetTransform {
        entity: EntityId,
        transform: PluginTransform,
    },
    SetEntityName {
        entity: EntityId,
        name: String,
    },
    SetEntityVisible {
        entity: EntityId,
        visible: bool,
    },
    SpawnEntity(EntityDefinition),
    DespawnEntity(EntityId),
    ReparentEntity {
        entity: EntityId,
        new_parent: Option<EntityId>,
    },
    LoadAsset(String),
}

/// Host-side implementation of the editor API.
pub struct EditorApiImpl {
    // Project path (for asset operations)
    pub project_assets_path: Option<std::path::PathBuf>,

    // UI registrations (persistent) - track which plugin owns each
    pub menu_items: Vec<(MenuLocation, MenuItem, String)>,
    pub panels: Vec<(PanelDefinition, String)>,
    pub panel_contents: std::collections::HashMap<String, String>, // JSON strings
    pub panel_visible: std::collections::HashMap<String, bool>,
    pub inspectors: Vec<(String, InspectorDefinition, String)>,
    pub inspector_contents: std::collections::HashMap<String, String>, // JSON strings
    pub toolbar_items: Vec<(ToolbarItem, String)>,
    pub context_menus: Vec<(ContextMenuLocation, MenuItem, String)>,
    pub status_bar_items: std::collections::HashMap<String, (StatusBarItem, String)>,

    // Tabbed panels - plugin tabs docked alongside built-in panels
    pub tabs: Vec<(PluginTab, String)>,
    pub tab_contents: std::collections::HashMap<String, String>, // JSON strings
    pub active_tabs: std::collections::HashMap<TabLocation, String>,

    // Currently active plugin (set during plugin callbacks)
    pub current_plugin_id: Option<String>,

    // State snapshot (synced from Bevy each frame)
    pub selected_entity: Option<EntityId>,
    pub entity_transforms: std::collections::HashMap<EntityId, PluginTransform>,
    pub entity_names: std::collections::HashMap<EntityId, String>,
    pub entity_visibility: std::collections::HashMap<EntityId, bool>,
    pub entity_parents: std::collections::HashMap<EntityId, Option<EntityId>>,
    pub entity_children: std::collections::HashMap<EntityId, Vec<EntityId>>,

    // Undo/redo state
    pub can_undo: bool,
    pub can_redo: bool,
    pub pending_undo: usize,
    pub pending_redo: usize,

    // Pending operations (applied to Bevy after plugin update)
    pub pending_operations: Vec<PendingOperation>,

    // Events
    pub pending_ui_events: Vec<UiEvent>,
    pub subscriptions: Vec<EditorEventType>,
    pub outgoing_events: Vec<CustomEvent>,

    // Pub/Sub system - subscriptions per plugin
    pub plugin_subscriptions: std::collections::HashMap<String, Vec<String>>,
    // Pending published events (event_type, data_json, source_plugin_id)
    pub pending_published_events: Vec<(String, String, String)>,

    // Settings (persistent)
    pub settings: std::collections::HashMap<String, SettingValue>,
}

impl Default for EditorApiImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorApiImpl {
    pub fn new() -> Self {
        Self {
            project_assets_path: None,
            menu_items: Vec::new(),
            panels: Vec::new(),
            panel_contents: std::collections::HashMap::new(),
            panel_visible: std::collections::HashMap::new(),
            inspectors: Vec::new(),
            inspector_contents: std::collections::HashMap::new(),
            toolbar_items: Vec::new(),
            context_menus: Vec::new(),
            status_bar_items: std::collections::HashMap::new(),
            tabs: Vec::new(),
            tab_contents: std::collections::HashMap::new(),
            active_tabs: std::collections::HashMap::new(),
            current_plugin_id: None,
            selected_entity: None,
            entity_transforms: std::collections::HashMap::new(),
            entity_names: std::collections::HashMap::new(),
            entity_visibility: std::collections::HashMap::new(),
            entity_parents: std::collections::HashMap::new(),
            entity_children: std::collections::HashMap::new(),
            can_undo: false,
            can_redo: false,
            pending_undo: 0,
            pending_redo: 0,
            pending_operations: Vec::new(),
            pending_ui_events: Vec::new(),
            subscriptions: Vec::new(),
            outgoing_events: Vec::new(),
            plugin_subscriptions: std::collections::HashMap::new(),
            pending_published_events: Vec::new(),
            settings: std::collections::HashMap::new(),
        }
    }

    /// Set the current plugin ID (called before plugin callbacks).
    pub fn set_current_plugin(&mut self, plugin_id: Option<String>) {
        self.current_plugin_id = plugin_id;
    }

    /// Set the project assets path.
    pub fn set_project_assets_path(&mut self, path: Option<std::path::PathBuf>) {
        self.project_assets_path = path;
    }

    /// Get the project assets path.
    pub fn get_project_assets_path(&self) -> Option<&std::path::Path> {
        self.project_assets_path.as_deref()
    }

    /// Get the current plugin ID or a default.
    pub fn current_plugin(&self) -> String {
        self.current_plugin_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Take pending operations (called by sync system).
    pub fn take_pending_operations(&mut self) -> Vec<PendingOperation> {
        std::mem::take(&mut self.pending_operations)
    }

    /// Update state snapshot from Bevy.
    pub fn sync_from_bevy(
        &mut self,
        selected: Option<EntityId>,
        transforms: std::collections::HashMap<EntityId, PluginTransform>,
        names: std::collections::HashMap<EntityId, String>,
        visibility: std::collections::HashMap<EntityId, bool>,
        parents: std::collections::HashMap<EntityId, Option<EntityId>>,
        children: std::collections::HashMap<EntityId, Vec<EntityId>>,
    ) {
        self.selected_entity = selected;
        self.entity_transforms = transforms;
        self.entity_names = names;
        self.entity_visibility = visibility;
        self.entity_parents = parents;
        self.entity_children = children;
    }

    /// Get entity by name (returns first match).
    pub fn get_entity_by_name(&self, name: &str) -> Option<EntityId> {
        self.entity_names
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(id, _)| *id)
    }

    /// Get entity visibility.
    pub fn get_entity_visible(&self, entity: EntityId) -> Option<bool> {
        self.entity_visibility.get(&entity).copied()
    }

    /// Get entity parent.
    pub fn get_entity_parent(&self, entity: EntityId) -> Option<Option<EntityId>> {
        self.entity_parents.get(&entity).copied()
    }

    /// Get entity children.
    pub fn get_entity_children(&self, entity: EntityId) -> Option<&Vec<EntityId>> {
        self.entity_children.get(&entity)
    }

    /// Push a UI event for plugins to receive.
    pub fn push_ui_event(&mut self, event: UiEvent) {
        self.pending_ui_events.push(event);
    }

    /// Remove all UI elements registered by a specific plugin.
    pub fn remove_plugin_elements(&mut self, plugin_id: &str) {
        self.menu_items.retain(|(_, _, id)| id != plugin_id);
        self.panels.retain(|(_, id)| id != plugin_id);
        self.inspectors.retain(|(_, _, id)| id != plugin_id);
        self.toolbar_items.retain(|(_, id)| id != plugin_id);
        self.context_menus.retain(|(_, _, id)| id != plugin_id);
        self.status_bar_items.retain(|_, (_, id)| id != plugin_id);

        // Remove panel contents for panels owned by this plugin
        let panel_ids: Vec<_> = self
            .panels
            .iter()
            .filter(|(_, id)| id == plugin_id)
            .map(|(p, _)| p.id.clone())
            .collect();
        for panel_id in panel_ids {
            self.panel_contents.remove(&panel_id);
        }

        // Remove tab contents for tabs owned by this plugin
        let tab_ids: Vec<_> = self
            .tabs
            .iter()
            .filter(|(_, id)| id == plugin_id)
            .map(|(t, _)| t.id.clone())
            .collect();
        for tab_id in tab_ids {
            self.tab_contents.remove(&tab_id);
        }
        self.tabs.retain(|(_, id)| id != plugin_id);
    }

    /// Clear all registered UI elements.
    pub fn clear(&mut self) {
        self.menu_items.clear();
        self.panels.clear();
        self.panel_contents.clear();
        self.panel_visible.clear();
        self.inspectors.clear();
        self.inspector_contents.clear();
        self.toolbar_items.clear();
        self.context_menus.clear();
        self.status_bar_items.clear();
        self.tabs.clear();
        self.tab_contents.clear();
        self.active_tabs.clear();
        self.current_plugin_id = None;
        self.can_undo = false;
        self.can_redo = false;
        self.pending_undo = 0;
        self.pending_redo = 0;
        self.pending_operations.clear();
        self.pending_ui_events.clear();
        self.subscriptions.clear();
        self.outgoing_events.clear();
        self.plugin_subscriptions.clear();
        self.pending_published_events.clear();
        // Keep settings - they persist across plugin reloads
    }

    /// Get tabs for a specific location.
    pub fn get_tabs_for_location(&self, location: TabLocation) -> Vec<&PluginTab> {
        self.tabs
            .iter()
            .filter(|(tab, _)| tab.location == location)
            .map(|(tab, _)| tab)
            .collect()
    }

    /// Get tab content (JSON string).
    pub fn get_tab_content(&self, tab_id: &str) -> Option<&str> {
        self.tab_contents.get(tab_id).map(|s| s.as_str())
    }

    /// Get active tab for a location.
    pub fn get_active_tab(&self, location: TabLocation) -> Option<&str> {
        self.active_tabs.get(&location).map(|s| s.as_str())
    }

    /// Set active tab for a location.
    pub fn set_active_tab(&mut self, location: TabLocation, tab_id: String) {
        self.active_tabs.insert(location, tab_id);
    }

    /// Clear active tab for a location (switch back to built-in).
    pub fn clear_active_tab(&mut self, location: TabLocation) {
        self.active_tabs.remove(&location);
    }

    // === Pub/Sub System ===

    /// Subscribe a plugin to an event type.
    pub fn subscribe_plugin(&mut self, plugin_id: &str, event_type: &str) {
        let subs = self
            .plugin_subscriptions
            .entry(plugin_id.to_string())
            .or_default();
        if !subs.contains(&event_type.to_string()) {
            subs.push(event_type.to_string());
        }
    }

    /// Unsubscribe a plugin from an event type.
    pub fn unsubscribe_plugin(&mut self, plugin_id: &str, event_type: &str) {
        if let Some(subs) = self.plugin_subscriptions.get_mut(plugin_id) {
            subs.retain(|s| s != event_type);
        }
    }

    /// Check if a plugin is subscribed to an event type (supports wildcards).
    pub fn is_subscribed(&self, plugin_id: &str, event_type: &str) -> bool {
        if let Some(subs) = self.plugin_subscriptions.get(plugin_id) {
            for sub in subs {
                if sub == event_type {
                    return true;
                }
                // Wildcard matching: "ui.*" matches "ui.button_clicked"
                if sub.ends_with(".*") {
                    let prefix = &sub[..sub.len() - 1]; // Remove "*"
                    if event_type.starts_with(prefix) {
                        return true;
                    }
                }
                // Match all
                if sub == "*" {
                    return true;
                }
            }
        }
        false
    }

    /// Publish an event from a plugin.
    pub fn publish_event(&mut self, source_plugin: &str, event_type: &str, data_json: &str) {
        self.pending_published_events.push((
            event_type.to_string(),
            data_json.to_string(),
            source_plugin.to_string(),
        ));
    }

    /// Take pending published events.
    pub fn take_published_events(&mut self) -> Vec<(String, String, String)> {
        std::mem::take(&mut self.pending_published_events)
    }
}
