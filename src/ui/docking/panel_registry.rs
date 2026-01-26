//! Panel registry for the docking system
//!
//! Manages registered panels and provides a unified interface for rendering panel content.

use super::dock_tree::PanelId;
use bevy_egui::egui::{self, Rect, Ui};
use std::collections::HashSet;

/// Minimum size constraints for panels
pub struct PanelConstraints {
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
}

impl Default for PanelConstraints {
    fn default() -> Self {
        Self {
            min_width: 100.0,
            min_height: 100.0,
            max_width: None,
            max_height: None,
        }
    }
}

impl PanelConstraints {
    pub fn with_min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    pub fn with_min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }
}

/// Tracks which panels are available (not closed)
#[derive(Debug, Clone, Default)]
pub struct PanelAvailability {
    /// Panels that have been closed and can be re-opened
    pub closed_panels: HashSet<PanelId>,
}

impl PanelAvailability {
    /// Check if a panel is currently available (not closed)
    pub fn is_available(&self, panel: &PanelId) -> bool {
        !self.closed_panels.contains(panel)
    }

    /// Close a panel
    pub fn close_panel(&mut self, panel: PanelId) {
        self.closed_panels.insert(panel);
    }

    /// Re-open a previously closed panel
    pub fn open_panel(&mut self, panel: &PanelId) {
        self.closed_panels.remove(panel);
    }

    /// Get list of closed panels
    pub fn get_closed_panels(&self) -> Vec<PanelId> {
        self.closed_panels.iter().cloned().collect()
    }
}

/// Get constraints for a specific panel type
pub fn get_panel_constraints(panel: &PanelId) -> PanelConstraints {
    match panel {
        PanelId::Hierarchy => PanelConstraints::default().with_min_width(150.0),
        PanelId::Inspector => PanelConstraints::default().with_min_width(250.0),
        PanelId::Assets => PanelConstraints::default().with_min_height(100.0),
        PanelId::Console => PanelConstraints::default().with_min_height(80.0),
        PanelId::Viewport => PanelConstraints::default().with_min_width(200.0).with_min_height(200.0),
        PanelId::Animation => PanelConstraints::default().with_min_height(100.0),
        PanelId::ScriptEditor => PanelConstraints::default().with_min_width(300.0).with_min_height(200.0),
        PanelId::History => PanelConstraints::default().with_min_width(200.0),
        PanelId::Plugin(_) => PanelConstraints::default(),
    }
}

/// All built-in panel types
pub fn all_builtin_panels() -> Vec<PanelId> {
    vec![
        PanelId::Hierarchy,
        PanelId::Inspector,
        PanelId::Assets,
        PanelId::Console,
        PanelId::Viewport,
        PanelId::Animation,
        PanelId::ScriptEditor,
        PanelId::History,
    ]
}

/// Render a placeholder for an unknown panel type
pub fn render_placeholder_panel(ui: &mut Ui, panel: &PanelId, rect: Rect) {
    let _response = ui.allocate_rect(rect, egui::Sense::hover());

    ui.painter().rect_filled(
        rect,
        0.0,
        egui::Color32::from_gray(40),
    );

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{} (placeholder)", panel.title()),
        egui::FontId::default(),
        egui::Color32::from_gray(120),
    );
}
