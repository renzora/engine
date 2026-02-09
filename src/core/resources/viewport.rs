#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui::TextureId;

use crate::viewport::ViewportMode;
use bevy::math::Vec2;

/// Bottom panel tab selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BottomPanelTab {
    #[default]
    Assets,
    Console,
    Animation,
}

/// Right panel tab selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPanelTab {
    #[default]
    Inspector,
    History,
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
    /// Currently selected right panel tab
    pub right_panel_tab: RightPanelTab,
    /// Current viewport mode (2D or 3D)
    pub viewport_mode: ViewportMode,
    /// Whether the bottom panel is minimized (showing only the bar)
    pub bottom_panel_minimized: bool,
    /// Previous height before minimizing (for restore)
    pub bottom_panel_prev_height: f32,
    /// Whether the camera is being dragged (to prevent selection on release)
    pub camera_dragging: bool,
    /// Whether left-click drag for camera is disabled (for terrain brush tools)
    pub disable_left_click_drag: bool,
    /// Whether a panel resize handle is being interacted with (to prevent viewport interaction)
    pub resize_handle_active: bool,
    /// Studio preview texture ID (from StudioPreviewPlugin)
    pub studio_preview_texture_id: Option<TextureId>,
    /// Studio preview texture size
    pub studio_preview_size: (u32, u32),
    /// Viewport right-click context menu position (screen coords)
    pub context_menu_pos: Option<Vec2>,
    /// Clipboard entity for copy/paste in viewport
    pub clipboard_entity: Option<Entity>,
    /// Cursor position when right-click started (for click-vs-drag detection)
    pub right_click_origin: Option<Vec2>,
    /// Whether the mouse moved during a right-click hold
    pub right_click_moved: bool,
    /// Currently open submenu in viewport context menu
    pub context_submenu: Option<String>,
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
            right_panel_tab: RightPanelTab::Inspector,
            viewport_mode: ViewportMode::default(),
            bottom_panel_minimized: false,
            bottom_panel_prev_height: 200.0,
            camera_dragging: false,
            disable_left_click_drag: false,
            resize_handle_active: false,
            studio_preview_texture_id: None,
            studio_preview_size: (512, 512),
            context_menu_pos: None,
            clipboard_entity: None,
            right_click_origin: None,
            right_click_moved: false,
            context_submenu: None,
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
