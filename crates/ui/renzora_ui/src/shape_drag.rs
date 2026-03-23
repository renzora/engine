//! Shape drag-and-drop state — tracks shapes being dragged from the shape library
//! to the viewport for placement via raycast.

use bevy::prelude::*;

/// Persistent resource for shape library drag-and-drop state.
///
/// Fields are set by the panel UI and viewport code, then polled by a Bevy system
/// that handles spawning. This avoids deferred command timing issues.
#[derive(Resource, Default)]
pub struct ShapeDragState {
    /// Shape currently being dragged from the panel (registry ID).
    pub dragging_shape: Option<&'static str>,
    /// Pending shape to spawn: (shape_id, position, normal).
    /// Set by the viewport drop handler, consumed by the spawn system.
    pub pending_drop: Option<PendingShapeDrop>,
    /// Ground plane (Y=0) intersection while dragging over viewport.
    pub drag_ground_position: Option<Vec3>,
    /// Surface raycast hit position (overrides ground plane).
    pub drag_surface_position: Option<Vec3>,
    /// Surface normal at raycast hit.
    pub drag_surface_normal: Vec3,
}

/// A shape drop waiting to be spawned.
pub struct PendingShapeDrop {
    pub shape_id: &'static str,
    pub position: Vec3,
    pub normal: Vec3,
}

/// Tracks the drag preview entity lifecycle.
#[derive(Resource, Default)]
pub struct ShapeDragPreviewState {
    /// The preview entity, if active.
    pub preview_entity: Option<Entity>,
    /// Which shape the preview is currently showing.
    pub preview_shape_id: Option<&'static str>,
}

/// Marker component for the shape drag preview entity (excluded from raycast).
#[derive(Component)]
pub struct ShapeDragPreview;
