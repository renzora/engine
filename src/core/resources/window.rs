#![allow(dead_code)]

use bevy::prelude::*;

/// Which edge(s) of the window are being resized
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ResizeEdge {
    #[default]
    None,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Window management state for custom title bar
#[derive(Resource, Default)]
pub struct WindowState {
    /// Whether the window is currently maximized
    pub is_maximized: bool,
    /// Request to close the window
    pub request_close: bool,
    /// Request to minimize the window
    pub request_minimize: bool,
    /// Request to toggle maximize state
    pub request_toggle_maximize: bool,
    /// Request to start window drag
    pub start_drag: bool,
    /// Whether the window is currently being dragged (manual fallback)
    pub is_being_dragged: bool,
    /// Drag offset for manual window dragging
    pub drag_offset: Option<(f32, f32)>,
    /// Which edge is being resized
    pub resize_edge: ResizeEdge,
    /// Whether the window is currently being resized
    pub is_resizing: bool,
    /// Initial window rect when resize started
    pub resize_start_rect: Option<(i32, i32, u32, u32)>, // x, y, width, height
    /// Initial cursor position when resize started
    pub resize_start_cursor: Option<(i32, i32)>,
}

impl WindowState {
    /// Request the window to close
    pub fn close(&mut self) {
        self.request_close = true;
    }

    /// Request the window to minimize
    pub fn minimize(&mut self) {
        self.request_minimize = true;
    }

    /// Request the window to toggle maximize
    pub fn toggle_maximize(&mut self) {
        self.request_toggle_maximize = true;
    }

    /// Clear all pending requests (called after handling)
    pub fn clear_requests(&mut self) {
        self.request_close = false;
        self.request_minimize = false;
        self.request_toggle_maximize = false;
        self.start_drag = false;
    }
}
