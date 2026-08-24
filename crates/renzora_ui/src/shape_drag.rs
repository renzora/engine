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
    /// Pending shape to spawn. Set by the viewport drop handler, consumed by
    /// the spawn system.
    pub pending_drop: Option<PendingShapeDrop>,
    /// Ground plane (Y=0) intersection while dragging over viewport.
    pub drag_ground_position: Option<Vec3>,
    /// Surface raycast hit position (overrides ground plane).
    pub drag_surface_position: Option<Vec3>,
    /// Surface normal at raycast hit.
    pub drag_surface_normal: Vec3,
    /// Where the drag preview is actually standing: the hit point lifted off
    /// the surface and rounded onto the translate-snap grid. The drop commits
    /// this verbatim, so the shape lands exactly where the ghost was — without
    /// it the release handler would re-derive the position and lose the snap.
    pub preview_position: Option<Vec3>,
    /// True when the drag was started from the bevy_ui shape library (so the
    /// native release handler owns the drop, not the egui viewport's `ui()`).
    pub native_drag: bool,
}

/// A shape drop waiting to be spawned.
pub struct PendingShapeDrop {
    pub shape_id: &'static str,
    /// The final world position to spawn at — surface offset and grid snap
    /// already applied by the preview, so the spawn system just uses it.
    pub position: Vec3,
}

/// Tracks the drag preview entity lifecycle.
#[derive(Resource, Default)]
pub struct ShapeDragPreviewState {
    /// The preview entity, if active.
    pub preview_entity: Option<Entity>,
    /// Which shape the preview is currently showing.
    pub preview_shape_id: Option<&'static str>,
    /// The preview mesh's local AABB min corner relative to its pivot, cached
    /// when the ghost is spawned. Edge snapping needs it to put the shape's
    /// bottom-left corner on a gridline, and computing it from the mesh at
    /// spawn time means it is right on the very first frame — the entity's
    /// `Aabb` component doesn't exist until Bevy calculates bounds a frame
    /// later, which would make the ghost visibly jump.
    pub preview_min_offset: Vec3,
}

/// Marker component for the shape drag preview entity (excluded from raycast).
#[derive(Component)]
pub struct ShapeDragPreview;
