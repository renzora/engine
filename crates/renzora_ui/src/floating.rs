//! Floating (undocked) panel windows — pure data.
//!
//! Panels dragged outside the dock tree become floating windows. The state
//! lives here; rendering is owned by the native (bevy_ui) shell.

use bevy::prelude::*;

/// A single floating (undocked) panel window.
#[derive(Debug, Clone)]
pub struct FloatingPanel {
    pub panel_id: String,
    pub pos: Vec2,
    pub size: Vec2,
}

/// Resource holding all floating (undocked) panels.
#[derive(Resource, Default)]
pub struct FloatingPanels {
    pub panels: Vec<FloatingPanel>,
}

impl FloatingPanels {
    pub fn add(&mut self, panel_id: String, pos: Vec2, size: Vec2) {
        if self.panels.iter().any(|p| p.panel_id == panel_id) {
            return;
        }
        self.panels.push(FloatingPanel {
            panel_id,
            pos,
            size,
        });
    }

    pub fn remove(&mut self, panel_id: &str) -> Option<FloatingPanel> {
        if let Some(idx) = self.panels.iter().position(|p| p.panel_id == panel_id) {
            Some(self.panels.remove(idx))
        } else {
            None
        }
    }

    pub fn contains(&self, panel_id: &str) -> bool {
        self.panels.iter().any(|p| p.panel_id == panel_id)
    }
}
