//! Docking state resource
//!
//! Manages the current dock tree layout and drag-drop state.

use bevy::prelude::*;
use crate::ui::docking::{
    DockTree, DockingLayoutConfig, DragState, PanelAvailability, PanelId,
    WorkspaceLayout, builtin_layouts, default_layout,
};

/// Resource managing the docking system state
#[derive(Resource)]
pub struct DockingState {
    /// The current dock tree layout
    pub dock_tree: DockTree,
    /// State for ongoing drag operations
    pub drag_state: Option<DragState>,
    /// Available/closed panel tracking
    pub panel_availability: PanelAvailability,
    /// Saved layouts (builtin + custom)
    pub layouts: Vec<WorkspaceLayout>,
    /// Name of the currently active layout
    pub active_layout: String,
    /// Whether the layout has been modified from the saved state
    pub is_modified: bool,
    /// Layout config for persistence
    pub config: DockingLayoutConfig,
}

impl Default for DockingState {
    fn default() -> Self {
        let layouts = builtin_layouts();
        Self {
            dock_tree: default_layout(),
            drag_state: None,
            panel_availability: PanelAvailability::default(),
            layouts,
            active_layout: "Scene".to_string(),
            is_modified: false,
            config: DockingLayoutConfig::default(),
        }
    }
}

impl DockingState {
    /// Create a new docking state with the given layout
    #[allow(dead_code)]
    pub fn with_layout(layout: DockTree) -> Self {
        Self {
            dock_tree: layout,
            ..Default::default()
        }
    }

    /// Switch to a named layout
    pub fn switch_layout(&mut self, name: &str) -> bool {
        // Check builtin layouts first
        for layout in builtin_layouts() {
            if layout.name == name {
                self.dock_tree = layout.dock_tree;
                self.active_layout = name.to_string();
                self.is_modified = false;
                return true;
            }
        }

        // Check custom layouts
        for layout in &self.config.custom_layouts {
            if layout.name == name {
                self.dock_tree = layout.dock_tree.clone();
                self.active_layout = name.to_string();
                self.is_modified = false;
                return true;
            }
        }

        false
    }

    /// Save the current layout with a name
    #[allow(dead_code)]
    pub fn save_layout(&mut self, name: String) {
        self.config.save_custom_layout(name.clone(), self.dock_tree.clone());
        self.active_layout = name;
        self.is_modified = false;

        // Add to layouts list if not already there
        if !self.layouts.iter().any(|l| l.name == self.active_layout) {
            self.layouts.push(WorkspaceLayout::new(
                self.active_layout.clone(),
                self.dock_tree.clone(),
            ));
        }
    }

    /// Delete a custom layout
    #[allow(dead_code)]
    pub fn delete_layout(&mut self, name: &str) -> bool {
        // Can't delete builtin layouts
        if builtin_layouts().iter().any(|l| l.name == name) {
            return false;
        }

        self.config.delete_layout(name);
        self.layouts.retain(|l| l.name != name);

        // If we deleted the active layout, switch to default
        if self.active_layout == name {
            self.switch_layout("Default");
        }

        true
    }

    /// Reset to default layout
    #[allow(dead_code)]
    pub fn reset_layout(&mut self) {
        self.dock_tree = default_layout();
        self.active_layout = "Default".to_string();
        self.is_modified = false;
    }

    /// Mark the layout as modified
    pub fn mark_modified(&mut self) {
        self.is_modified = true;
    }

    /// Get all available layout names
    #[allow(dead_code)]
    pub fn layout_names(&self) -> Vec<String> {
        let mut names: Vec<String> = builtin_layouts()
            .iter()
            .map(|l| l.name.clone())
            .collect();
        names.extend(self.config.custom_layouts.iter().map(|l| l.name.clone()));
        names
    }

    /// Check if a panel is currently visible in the dock tree
    pub fn is_panel_visible(&self, panel: &PanelId) -> bool {
        self.dock_tree.contains_panel(panel)
    }

    /// Open a closed panel (adds it to the dock tree)
    pub fn open_panel(&mut self, panel: PanelId) {
        if self.is_panel_visible(&panel) {
            return;
        }

        self.panel_availability.open_panel(&panel);

        // Find a suitable place to add the panel
        // Try to add as a tab to a related panel
        let target = match &panel {
            PanelId::History | PanelId::Settings => Some(PanelId::Inspector),
            PanelId::Console | PanelId::Animation => Some(PanelId::Assets),
            PanelId::ScriptEditor => Some(PanelId::Viewport),
            _ => None,
        };

        if let Some(target_panel) = target {
            if self.dock_tree.add_tab(&target_panel, panel.clone()) {
                self.mark_modified();
                return;
            }
        }

        // If no suitable tab group, add as a split to viewport
        if self.dock_tree.split_at(
            &PanelId::Viewport,
            panel,
            crate::ui::docking::SplitDirection::Vertical,
            false,
        ) {
            self.mark_modified();
        }
    }

    /// Close a panel (removes from dock tree)
    pub fn close_panel(&mut self, panel: &PanelId) {
        if !panel.can_close() {
            return;
        }

        if self.dock_tree.remove_panel(panel) {
            self.panel_availability.close_panel(panel.clone());
            self.mark_modified();
        }
    }

    /// Start dragging a panel
    pub fn start_drag(&mut self, panel: PanelId, pos: bevy_egui::egui::Pos2, panel_rect: bevy_egui::egui::Rect) {
        self.drag_state = Some(DragState::new(panel, pos, panel_rect));
    }

    /// End the current drag operation
    pub fn end_drag(&mut self) {
        self.drag_state = None;
    }

    /// Load from config
    pub fn load_from_config(&mut self, config: DockingLayoutConfig) {
        self.config = config;

        // Load the current tree if saved
        if let Some(tree) = self.config.current_tree.clone() {
            self.dock_tree = tree;
        }

        // Set active layout
        if !self.config.active_layout.is_empty() {
            self.active_layout = self.config.active_layout.clone();
        }

        // Rebuild layouts list
        self.layouts = builtin_layouts();
        self.layouts.extend(self.config.custom_layouts.clone());
    }

    /// Save to config
    pub fn save_to_config(&self) -> DockingLayoutConfig {
        DockingLayoutConfig {
            active_layout: self.active_layout.clone(),
            custom_layouts: self.config.custom_layouts.clone(),
            current_tree: Some(self.dock_tree.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = DockingState::default();
        assert_eq!(state.active_layout, "Default");
        assert!(!state.is_modified);
        assert!(state.is_panel_visible(&PanelId::Viewport));
    }

    #[test]
    fn test_switch_layout() {
        let mut state = DockingState::default();
        assert!(state.switch_layout("Debug"));
        assert_eq!(state.active_layout, "Debug");
    }

    #[test]
    fn test_save_custom_layout() {
        let mut state = DockingState::default();
        state.save_layout("My Layout".to_string());
        assert_eq!(state.active_layout, "My Layout");
        assert!(!state.is_modified);
    }
}
