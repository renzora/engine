//! The gizmo's vocabulary — which axis, which mesh part, which viewport slot —
//! plus the two resources that carry drag and box-selection state.

use bevy::prelude::*;

pub use renzora_editor_framework::{GizmoMode, GizmoSpace};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
    XY,
    XZ,
    YZ,
}

impl GizmoAxis {
    pub(crate) fn direction(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
            Self::XY => Vec3::Z,
            Self::XZ => Vec3::Y,
            Self::YZ => Vec3::X,
        }
    }

    /// Axis direction with per-axis signs applied so single-axis handles
    /// (X/Y/Z) flip to face the camera. Plane normals are left alone —
    /// the drag plane is the same regardless of viewing side.
    pub(crate) fn signed_direction(self, signs: Vec3) -> Vec3 {
        match self {
            Self::X => Vec3::new(signs.x, 0.0, 0.0),
            Self::Y => Vec3::new(0.0, signs.y, 0.0),
            Self::Z => Vec3::new(0.0, 0.0, signs.z),
            Self::XY | Self::XZ | Self::YZ => self.direction(),
        }
    }

    pub(crate) fn is_plane(self) -> bool {
        matches!(self, Self::XY | Self::XZ | Self::YZ)
    }

    pub(crate) fn plane_axes(self) -> Option<(Vec3, Vec3)> {
        match self {
            Self::XY => Some((Vec3::X, Vec3::Y)),
            Self::XZ => Some((Vec3::X, Vec3::Z)),
            Self::YZ => Some((Vec3::Y, Vec3::Z)),
            _ => None,
        }
    }

    /// Plane axes with `axis_signs` baked in so plane handles flip into the
    /// quadrant facing the camera, matching how single-axis arrows already
    /// flip via `signed_direction`. Used by the picking quads.
    pub(crate) fn signed_plane_axes(self, signs: Vec3) -> Option<(Vec3, Vec3)> {
        match self {
            Self::XY => Some((Vec3::new(signs.x, 0.0, 0.0), Vec3::new(0.0, signs.y, 0.0))),
            Self::XZ => Some((Vec3::new(signs.x, 0.0, 0.0), Vec3::new(0.0, 0.0, signs.z))),
            Self::YZ => Some((Vec3::new(0.0, signs.y, 0.0), Vec3::new(0.0, 0.0, signs.z))),
            _ => None,
        }
    }
}

pub(crate) const AXES: [GizmoAxis; 3] = [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z];
pub(crate) const PLANES: [GizmoAxis; 3] = [GizmoAxis::XY, GizmoAxis::XZ, GizmoAxis::YZ];

#[derive(Component)]
pub(crate) struct GizmoRoot;

#[derive(Component)]
pub(crate) struct GizmoMesh;

/// Which viewport slot a gizmo mesh set belongs to. There is one full gizmo
/// mesh set per slot (`0..VIEWPORT_COUNT`), each sitting on that slot's private
/// overlay render layer (`VIEWPORT_3D_GIZMO_LAYER_BASE + slot`) and scaled to
/// that slot's own camera — so every open viewport shows a correctly-sized
/// transform handle rather than one handle sized for the focused camera.
/// Interaction (hover / drag) still runs against the focused slot only; the
/// non-focused sets are purely visual.
#[derive(Component, Clone, Copy)]
pub(crate) struct GizmoSlot(pub usize);

/// Per-slot transform-gizmo draw parameters, filled by `update_gizmo_transforms`
/// (which already sizes each slot's mesh set from its own camera) and read by
/// `draw_line_gizmos` so the immediate-mode rotate / scale / plane lines match
/// each slot's mesh handle exactly. Keyed by viewport slot index.
#[derive(Resource)]
pub(crate) struct PerSlotGizmo {
    /// World-space scale factor of this slot's gizmo (distance-to-camera based).
    pub scale: [f32; renzora::core::viewport_types::VIEWPORT_COUNT],
    /// Per-axis camera-facing signs for this slot (X/Z flip toward its camera).
    pub signs: [Vec3; renzora::core::viewport_types::VIEWPORT_COUNT],
    /// Whether this slot should draw the tool-mode line gizmos this frame
    /// (docked, has a live camera, and the shared show/hide gates pass).
    pub draw: [bool; renzora::core::viewport_types::VIEWPORT_COUNT],
}

impl Default for PerSlotGizmo {
    fn default() -> Self {
        use renzora::core::viewport_types::VIEWPORT_COUNT;
        Self {
            scale: [1.0; VIEWPORT_COUNT],
            signs: [Vec3::ONE; VIEWPORT_COUNT],
            draw: [false; VIEWPORT_COUNT],
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GizmoPart {
    XShaft,
    XHead,
    YShaft,
    YHead,
    ZShaft,
    ZHead,
    XScaleCube,
    YScaleCube,
    ZScaleCube,
    Center,
}

impl GizmoPart {
    pub(crate) fn axis(self) -> Option<GizmoAxis> {
        match self {
            Self::XShaft | Self::XHead | Self::XScaleCube => Some(GizmoAxis::X),
            Self::YShaft | Self::YHead | Self::YScaleCube => Some(GizmoAxis::Y),
            Self::ZShaft | Self::ZHead | Self::ZScaleCube => Some(GizmoAxis::Z),
            Self::Center => None,
        }
    }

    pub(crate) fn is_translate_only(self) -> bool {
        matches!(self, Self::XHead | Self::YHead | Self::ZHead)
    }

    pub(crate) fn is_scale_only(self) -> bool {
        matches!(self, Self::XScaleCube | Self::YScaleCube | Self::ZScaleCube)
    }
}

#[derive(Resource)]
pub struct GizmoState {
    pub active_axis: Option<GizmoAxis>,
    pub hovered_axis: Option<GizmoAxis>,
    pub drag_starts: Vec<(Entity, Vec3, Quat, Vec3)>,
    pub drag_offset: Vec3,
    pub drag_angle: f32,
    /// Snapped counterpart of [`Self::drag_angle`] for the rotate HUD. Stores
    /// the same value the drag handler applied to the entity so the pie-sector
    /// and degrees label read in step increments instead of scrolling through
    /// every intermediate degree. `0.0` when no rotate drag is active.
    pub drag_angle_snapped: f32,
    pub drag_scale_factor: f32,
    pub gizmo_scale: f32,
    /// +1 or -1 per axis — flipped so each arrow points toward the camera
    /// rather than away, keeping handles visible and pickable regardless of
    /// the current viewing angle. Locked while a drag is in progress so
    /// the handle direction doesn't flip mid-drag.
    pub axis_signs: Vec3,
    /// World-space orientation of the gizmo handles, captured at drag start so
    /// the axes stay fixed for the whole gesture even in Local space (where the
    /// object's rotation — and thus the live basis — changes as you rotate it).
    pub drag_basis: Quat,
    /// World-space pivot the active drag rotates/scales about (the selection's
    /// AABB center at drag start), so the object pivots in place.
    pub drag_pivot: Vec3,
    /// Each dragged entity's parent world affine, captured at drag start (the
    /// parent doesn't move during the gesture). World-space deltas are converted
    /// into this frame before being written to the entity's local `Transform`,
    /// so transforms are correct under any nesting. Index-aligned with
    /// `drag_starts`.
    pub drag_parents: Vec<bevy::math::Affine3A>,
    /// World point under the cursor at drag start, projected onto the dragged
    /// axis line / plane. Translate keeps this point pinned to the cursor each
    /// frame so the gizmo tracks the pointer exactly instead of drifting.
    pub drag_grab: Vec3,
}

impl Default for GizmoState {
    fn default() -> Self {
        Self {
            active_axis: None,
            hovered_axis: None,
            drag_starts: Vec::new(),
            drag_offset: Vec3::ZERO,
            drag_angle: 0.0,
            drag_angle_snapped: 0.0,
            drag_scale_factor: 0.0,
            gizmo_scale: 1.0,
            axis_signs: Vec3::ONE,
            drag_basis: Quat::IDENTITY,
            drag_pivot: Vec3::ZERO,
            drag_parents: Vec::new(),
            drag_grab: Vec3::ZERO,
        }
    }
}

/// World-space orientation of the gizmo handles for `mode`, given the
/// selection's world rotation and the active [`GizmoSpace`]. Scale handles are
/// always local-aligned — a non-uniform scale along world axes can't be written
/// back as a `Transform` (it would shear a rotated object) — so the space toggle
/// only changes which way the scale handles point, never the scale math.
pub(crate) fn gizmo_basis(space: GizmoSpace, mode: GizmoMode, sel_world_rot: Quat) -> Quat {
    match mode {
        GizmoMode::Scale => sel_world_rot,
        _ => space.basis(sel_world_rot),
    }
}

/// State for box/marquee selection (drag to select multiple entities).
///
/// A single click is also routed through this state: on press we arm
/// `active` + optionally remember the entity under the cursor in
/// `pending_pick`. On release, if the mouse barely moved, we commit the
/// pending pick (or deselect on empty space); if it moved past the drag
/// threshold, we finalise a box selection. This makes drag-select work
/// whether the drag starts on an entity or on empty space.
#[derive(Resource, Default, Clone, Copy)]
pub struct BoxSelectionState {
    /// Whether a click/drag gesture is in progress.
    pub active: bool,
    /// Start position in screen coordinates.
    pub start_pos: Vec2,
    /// Current position in screen coordinates.
    pub current_pos: Vec2,
    /// Entity under the cursor at press time. Committed as a single-entity
    /// selection on release if the gesture didn't become a drag.
    pub pending_pick: Option<Entity>,
}

impl BoxSelectionState {
    /// Get the selection rectangle as (min, max) screen positions.
    pub fn get_rect(&self) -> (Vec2, Vec2) {
        let min = Vec2::new(
            self.start_pos.x.min(self.current_pos.x),
            self.start_pos.y.min(self.current_pos.y),
        );
        let max = Vec2::new(
            self.start_pos.x.max(self.current_pos.x),
            self.start_pos.y.max(self.current_pos.y),
        );
        (min, max)
    }

    /// Check if the box is large enough to be considered a drag (not just a click).
    pub fn is_drag(&self) -> bool {
        let d = (self.current_pos - self.start_pos).abs();
        d.x > 5.0 || d.y > 5.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── GizmoAxis ───────────────────────────────────────────────────────────

    #[test]
    fn gizmo_axis_directions_and_plane_classification() {
        assert_eq!(GizmoAxis::X.direction(), Vec3::X);
        assert_eq!(GizmoAxis::Y.direction(), Vec3::Y);
        assert_eq!(GizmoAxis::Z.direction(), Vec3::Z);
        // Plane "direction" is the plane normal.
        assert_eq!(GizmoAxis::XY.direction(), Vec3::Z);
        assert_eq!(GizmoAxis::XZ.direction(), Vec3::Y);
        assert_eq!(GizmoAxis::YZ.direction(), Vec3::X);

        for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
            assert!(!axis.is_plane());
            assert!(axis.plane_axes().is_none());
        }
        for plane in [GizmoAxis::XY, GizmoAxis::XZ, GizmoAxis::YZ] {
            assert!(plane.is_plane());
        }
        assert_eq!(GizmoAxis::XY.plane_axes(), Some((Vec3::X, Vec3::Y)));
        assert_eq!(GizmoAxis::XZ.plane_axes(), Some((Vec3::X, Vec3::Z)));
        assert_eq!(GizmoAxis::YZ.plane_axes(), Some((Vec3::Y, Vec3::Z)));
    }

    #[test]
    fn gizmo_axis_signed_direction_flips_single_axes_only() {
        let signs = Vec3::new(-1.0, 1.0, -1.0);
        assert_eq!(GizmoAxis::X.signed_direction(signs), Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(GizmoAxis::Y.signed_direction(signs), Vec3::Y);
        assert_eq!(GizmoAxis::Z.signed_direction(signs), Vec3::new(0.0, 0.0, -1.0));
        // Plane normals are unaffected by signs.
        assert_eq!(GizmoAxis::XY.signed_direction(signs), Vec3::Z);
    }

    #[test]
    fn gizmo_axis_signed_plane_axes_bake_signs() {
        let signs = Vec3::new(-1.0, 1.0, -1.0);
        assert_eq!(
            GizmoAxis::XY.signed_plane_axes(signs),
            Some((Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)))
        );
        assert_eq!(
            GizmoAxis::YZ.signed_plane_axes(signs),
            Some((Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)))
        );
        assert_eq!(GizmoAxis::X.signed_plane_axes(signs), None);
    }

    // ── BoxSelectionState ───────────────────────────────────────────────────

    #[test]
    fn box_selection_get_rect_normalizes_inverted_drag() {
        let state = BoxSelectionState {
            active: true,
            start_pos: Vec2::new(100.0, 20.0),
            current_pos: Vec2::new(40.0, 80.0),
            pending_pick: None,
        };
        let (min, max) = state.get_rect();
        assert_eq!(min, Vec2::new(40.0, 20.0));
        assert_eq!(max, Vec2::new(100.0, 80.0));
    }

    #[test]
    fn box_selection_is_drag_requires_movement_past_threshold() {
        let mut state = BoxSelectionState {
            start_pos: Vec2::new(10.0, 10.0),
            current_pos: Vec2::new(10.0, 10.0),
            ..Default::default()
        };
        assert!(!state.is_drag());
        // Exactly 5px is still a click (threshold is strict >).
        state.current_pos = Vec2::new(15.0, 10.0);
        assert!(!state.is_drag());
        state.current_pos = Vec2::new(15.1, 10.0);
        assert!(state.is_drag());
        // Either axis alone is enough.
        state.current_pos = Vec2::new(10.0, 16.0);
        assert!(state.is_drag());
    }
}
