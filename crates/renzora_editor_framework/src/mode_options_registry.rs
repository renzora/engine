//! Context-sensitive header options keyed on [`ViewportMode`].
//!
//! When the viewport enters a mode (Edit, Sculpt, Paint, Animate), whichever
//! plugin owns that mode can take over the horizontal header strip to show
//! its own tools. Checked before [`ToolOptionsRegistry`]; if no mode drawer
//! is registered, the header falls back to tool-options then the default
//! strip.

use bevy::prelude::{Resource, World};
use bevy_egui::egui;
use renzora_core::viewport_types::ViewportMode;
use std::collections::HashMap;

pub type ModeOptionsDrawer = fn(&mut egui::Ui, &World);

#[derive(Resource, Default)]
pub struct ViewportModeOptionsRegistry {
    drawers: HashMap<ViewportMode, ModeOptionsDrawer>,
}

impl ViewportModeOptionsRegistry {
    /// Register a drawer for a mode. Overwrites any prior drawer.
    pub fn register(&mut self, mode: ViewportMode, drawer: ModeOptionsDrawer) {
        self.drawers.insert(mode, drawer);
    }

    pub fn drawer_for(&self, mode: ViewportMode) -> Option<ModeOptionsDrawer> {
        self.drawers.get(&mode).copied()
    }
}
