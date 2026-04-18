//! Context-sensitive viewport-toolbar options.
//!
//! Each active tool can register a renderer that replaces the default
//! inline contents of the viewport header (Photoshop-style options bar).
//! The right-hand dropdowns remain visible in all cases.

use bevy::prelude::{Resource, World};
use bevy_egui::egui;
use std::collections::HashMap;

use crate::ActiveTool;

/// A function that renders tool-specific inline options into the given UI.
pub type ToolOptionsDrawer = fn(&mut egui::Ui, &World);

#[derive(Resource, Default)]
pub struct ToolOptionsRegistry {
    drawers: HashMap<ActiveTool, ToolOptionsDrawer>,
}

impl ToolOptionsRegistry {
    /// Register a drawer for a tool. Overwrites any prior drawer for that tool.
    pub fn register(&mut self, tool: ActiveTool, drawer: ToolOptionsDrawer) {
        self.drawers.insert(tool, drawer);
    }

    pub fn drawer_for(&self, tool: ActiveTool) -> Option<ToolOptionsDrawer> {
        self.drawers.get(&tool).copied()
    }
}
