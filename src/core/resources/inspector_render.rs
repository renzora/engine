//! Inspector panel render state shared between UI systems
//!
//! This module provides a resource for coordinating Inspector panel rendering
//! between the regular `editor_ui` system (which handles layout/docking) and
//! an exclusive system that renders the actual inspector content with World access.

use bevy::prelude::*;
use bevy_egui::egui;
use std::path::PathBuf;

use crate::theming::Theme;

/// State for deferred Inspector panel rendering
///
/// The regular `editor_ui` system sets up the panel frame and stores this state,
/// then an exclusive system renders the actual content using `render_inspector_content_world`.
#[derive(Resource, Default)]
pub struct InspectorPanelRenderState {
    /// The content rect where inspector content should be rendered
    pub content_rect: Option<egui::Rect>,
    /// Whether the inspector panel is active and should be rendered
    pub should_render: bool,
    /// Camera preview texture ID (if available)
    pub camera_preview_texture_id: Option<egui::TextureId>,
    /// Whether rendering caused a scene change
    pub scene_changed: bool,
    /// Theme snapshot for rendering
    pub theme: Option<Theme>,
    /// Asset path being dragged (copied from AssetBrowserState before resource_scope)
    pub dragging_asset_path: Option<PathBuf>,
    /// Set by inspector when a drag-drop is consumed (signals AssetBrowserState to clear)
    pub drag_accepted: bool,
    /// Set by inspector for OS file drops that need importing to assets
    pub pending_file_import: Option<PathBuf>,
}

impl InspectorPanelRenderState {
    /// Reset state for next frame
    pub fn reset(&mut self) {
        self.content_rect = None;
        self.should_render = false;
        self.camera_preview_texture_id = None;
        self.scene_changed = false;
        self.theme = None;
        self.dragging_asset_path = None;
        self.drag_accepted = false;
        self.pending_file_import = None;
    }

    /// Mark the inspector for rendering with the given rect
    pub fn request_render(&mut self, rect: egui::Rect, camera_texture: Option<egui::TextureId>, theme: &Theme) {
        self.content_rect = Some(rect);
        self.should_render = true;
        self.camera_preview_texture_id = camera_texture;
        self.theme = Some(theme.clone());
    }
}
