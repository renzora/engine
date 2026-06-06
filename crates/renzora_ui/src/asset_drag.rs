//! Asset drag-and-drop state — tracks assets being dragged from the asset browser
//! to inspector drop targets, hierarchy items, or the viewport.

use std::path::PathBuf;

use bevy::prelude::*;

/// Active asset drag state — inserted as a resource when an asset drag begins,
/// removed on drop or cancel.
#[derive(Resource, Clone, Debug)]
pub struct AssetDragPayload {
    /// Full path to the asset being dragged (primary, for ghost + hover checks).
    pub path: PathBuf,
    /// All paths in a multi-select drag (includes `path` as the first entry).
    /// When the drag is a single item this is `vec![path.clone()]`.
    pub paths: Vec<PathBuf>,
    /// Display name (filename).
    pub name: String,
    /// Phosphor icon string for this file type.
    pub icon: String,
    /// Accent color for this file type (RGB).
    pub color: [u8; 3],
    /// Screen position where the drag started.
    pub origin: Vec2,
    /// True once the pointer moves >5px from origin.
    pub is_detached: bool,
    /// Number of items being dragged (1 = single file, >1 = multi-select).
    pub drag_count: usize,
}

impl AssetDragPayload {
    /// File extension (lowercase, no dot). Empty string if none.
    pub fn extension(&self) -> String {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
    }

    /// Check if this payload's extension matches any in the given list.
    pub fn matches_extensions(&self, extensions: &[&str]) -> bool {
        if extensions.is_empty() {
            return true; // empty = accept all
        }
        let ext = self.extension();
        extensions
            .iter()
            .any(|&allowed| ext.eq_ignore_ascii_case(allowed))
    }
}
