use bevy::prelude::*;

/// Bottom panel tab selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BottomPanelTab {
    #[default]
    Assets,
    Console,
}

/// Tracks viewport state and layout
#[derive(Resource)]
pub struct ViewportState {
    /// Size of the viewport in pixels
    pub size: [f32; 2],
    /// Position of the viewport in window coordinates
    pub position: [f32; 2],
    /// Whether the mouse is hovering over the viewport
    pub hovered: bool,
    /// Panel sizes (managed manually for persistence)
    pub hierarchy_width: f32,
    pub inspector_width: f32,
    pub assets_height: f32,
    /// Currently selected bottom panel tab
    pub bottom_panel_tab: BottomPanelTab,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            size: [1050.0, 881.0],
            position: [260.0, 56.0],
            hovered: false,
            hierarchy_width: 260.0,
            inspector_width: 320.0,
            assets_height: 200.0,
            bottom_panel_tab: BottomPanelTab::Assets,
        }
    }
}

impl ViewportState {
    /// Get the aspect ratio of the viewport
    pub fn aspect_ratio(&self) -> f32 {
        if self.size[1] > 0.0 {
            self.size[0] / self.size[1]
        } else {
            1.0
        }
    }

    /// Check if a point is within the viewport
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.position[0]
            && x <= self.position[0] + self.size[0]
            && y >= self.position[1]
            && y <= self.position[1] + self.size[1]
    }
}
