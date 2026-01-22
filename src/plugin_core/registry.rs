//! Plugin registry for tracking registered plugin components.

use std::collections::HashMap;

use bevy::prelude::*;

use super::api::{ContextMenuLocation, InspectorDefinition, MenuItem, MenuLocation, PanelDefinition, ToolbarItem};
use crate::ui_api::types::UiId;
use crate::ui_api::widgets::Widget;

/// Registry for all plugin-registered UI elements
#[derive(Resource, Default)]
pub struct PluginRegistry {
    /// Menu items registered by plugins
    pub menu_items: HashMap<MenuLocation, Vec<(String, MenuItem)>>,
    /// Panels registered by plugins
    pub panels: HashMap<String, RegisteredPanel>,
    /// Inspector sections registered by plugins
    pub inspectors: HashMap<String, Vec<(String, InspectorDefinition)>>,
    /// Toolbar items registered by plugins
    pub toolbar_items: Vec<(String, ToolbarItem)>,
    /// Context menu items registered by plugins
    pub context_menus: HashMap<ContextMenuLocation, Vec<(String, MenuItem)>>,
}

/// A registered panel with its current content
pub struct RegisteredPanel {
    pub definition: PanelDefinition,
    pub plugin_id: String,
    pub content: Vec<Widget>,
    pub visible: bool,
}

impl PluginRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a menu item from a plugin
    pub fn register_menu_item(&mut self, plugin_id: &str, location: MenuLocation, item: MenuItem) {
        self.menu_items
            .entry(location)
            .or_default()
            .push((plugin_id.to_string(), item));
    }

    /// Register a panel from a plugin
    pub fn register_panel(&mut self, plugin_id: &str, definition: PanelDefinition) {
        let id = definition.id.clone();
        self.panels.insert(
            id,
            RegisteredPanel {
                definition,
                plugin_id: plugin_id.to_string(),
                content: Vec::new(),
                visible: true,
            },
        );
    }

    /// Set panel content
    pub fn set_panel_content(&mut self, panel_id: &str, content: Vec<Widget>) {
        if let Some(panel) = self.panels.get_mut(panel_id) {
            panel.content = content;
        }
    }

    /// Register an inspector section from a plugin
    pub fn register_inspector(&mut self, plugin_id: &str, type_id: &str, inspector: InspectorDefinition) {
        self.inspectors
            .entry(type_id.to_string())
            .or_default()
            .push((plugin_id.to_string(), inspector));
    }

    /// Register a toolbar item from a plugin
    pub fn register_toolbar_item(&mut self, plugin_id: &str, item: ToolbarItem) {
        self.toolbar_items.push((plugin_id.to_string(), item));
    }

    /// Register a context menu item from a plugin
    pub fn register_context_menu(&mut self, plugin_id: &str, context: ContextMenuLocation, item: MenuItem) {
        self.context_menus
            .entry(context)
            .or_default()
            .push((plugin_id.to_string(), item));
    }

    /// Unregister all items from a plugin
    pub fn unregister_plugin(&mut self, plugin_id: &str) {
        // Remove menu items
        for items in self.menu_items.values_mut() {
            items.retain(|(id, _)| id != plugin_id);
        }

        // Remove panels
        self.panels.retain(|_, panel| panel.plugin_id != plugin_id);

        // Remove inspectors
        for items in self.inspectors.values_mut() {
            items.retain(|(id, _)| id != plugin_id);
        }

        // Remove toolbar items
        self.toolbar_items.retain(|(id, _)| id != plugin_id);

        // Remove context menu items
        for items in self.context_menus.values_mut() {
            items.retain(|(id, _)| id != plugin_id);
        }
    }

    /// Get menu items for a location
    pub fn get_menu_items(&self, location: &MenuLocation) -> Vec<&MenuItem> {
        self.menu_items
            .get(location)
            .map(|items| items.iter().map(|(_, item)| item).collect())
            .unwrap_or_default()
    }

    /// Get all visible panels
    pub fn get_visible_panels(&self) -> Vec<(&str, &RegisteredPanel)> {
        self.panels
            .iter()
            .filter(|(_, panel)| panel.visible)
            .map(|(id, panel)| (id.as_str(), panel))
            .collect()
    }

    /// Toggle panel visibility
    pub fn toggle_panel_visibility(&mut self, panel_id: &str) {
        if let Some(panel) = self.panels.get_mut(panel_id) {
            panel.visible = !panel.visible;
        }
    }

    /// Get inspector sections for a component type
    pub fn get_inspectors(&self, type_id: &str) -> Vec<&InspectorDefinition> {
        self.inspectors
            .get(type_id)
            .map(|items| items.iter().map(|(_, item)| item).collect())
            .unwrap_or_default()
    }

    /// Get context menu items for a context
    pub fn get_context_menu_items(&self, context: &ContextMenuLocation) -> Vec<&MenuItem> {
        self.context_menus
            .get(context)
            .map(|items| items.iter().map(|(_, item)| item).collect())
            .unwrap_or_default()
    }
}
