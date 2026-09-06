//! Borrowing the transform gizmo, so a plugin can drive something that is not
//! an entity's `Transform`.
//!
//! # Why this exists
//!
//! The gizmo in `renzora_gizmo` follows [`EditorSelection`](super::EditorSelection)
//! and writes `Transform`s. That is right for the thing it was built for and
//! useless for everything else: a mesh editor moves *vertices*, a rig editor
//! moves *bones*, a collider editor moves a shape offset. None of those is an
//! entity transform, so none of them could use the gizmo at all.
//!
//! The obvious answer — each plugin draws its own handles — is the wrong one.
//! The hard parts of a gizmo are not the arrows: they are the analytic hit test,
//! the always-on-top material, sizing the handles per viewport so they stay a
//! constant screen size, honouring snap, and the Local/World basis. A second
//! implementation gets a different look and a worse hit test, and the two drift.
//!
//! So the gizmo stays in one place and this turns it around. A plugin says
//! *where* the handles go; the gizmo says *what the drag did*; the plugin
//! decides what that means and records its own undo.
//!
//! # Contract
//!
//! While [`mode`](GizmoTarget::mode) is anything but [`GizmoMode::None`]:
//!
//! - the gizmo draws at [`pivot`](GizmoTarget::pivot) with
//!   [`basis`](GizmoTarget::basis) instead of at the selection's pivot,
//! - it writes no `Transform` and records no undo,
//! - it accumulates the drag into [`translation`](GizmoTarget::translation) /
//!   [`rotation`](GizmoTarget::rotation) / [`scale`](GizmoTarget::scale), each
//!   measured **from where the drag started**, not per frame. A plugin
//!   snapshots its geometry on [`drag_started`](GizmoTarget::drag_started) and
//!   applies the delta to that snapshot every frame, which is also what makes
//!   the drag cancellable and free of accumulated float drift.
//!
//! # Who owns the mouse
//!
//! Borrowing the gizmo does **not** re-enable click-picking or box selection.
//! Those disengage on [`ActiveTool::None`](super::ActiveTool::None), which is
//! what a plugin driving viewport input already sets — and it must keep setting
//! it, or its own picking and the editor's will both act on the same click.
//! This is the one part a plugin author has to get right, and it is why the two
//! switches are separate: `ActiveTool` decides who owns the *mouse*, this
//! decides who owns the *handles*.

use bevy::prelude::*;

use super::tools::GizmoMode;

/// A plugin's claim on the transform gizmo. See the module docs.
///
/// Default is "not borrowed", so an editor with no such plugin behaves exactly
/// as it did before this existed.
#[derive(Resource, Clone, Copy, Debug)]
pub struct GizmoTarget {
    /// Which handles to show. [`GizmoMode::None`] releases the gizmo back to
    /// the entity selection; [`GizmoMode::Select`] is treated the same way,
    /// since there is nothing for a borrowed gizmo to select.
    pub mode: GizmoMode,
    /// Where the handles sit, in world space.
    pub pivot: Vec3,
    /// The orientation the handles take in Local space. `IDENTITY` makes Local
    /// and World the same thing, which is the right default for a selection of
    /// loose vertices that has no orientation of its own.
    pub basis: Quat,

    /// Movement since the drag began, in world space.
    pub translation: Vec3,
    /// Rotation since the drag began, about [`pivot`](Self::pivot).
    pub rotation: Quat,
    /// Scale since the drag began, about [`pivot`](Self::pivot).
    pub scale: Vec3,

    /// A handle is being dragged right now.
    pub dragging: bool,
    /// Set for the single frame a drag begins. Snapshot on this.
    pub drag_started: bool,
    /// Set for the single frame a drag ends. Record undo on this.
    pub drag_ended: bool,
}

impl Default for GizmoTarget {
    fn default() -> Self {
        Self {
            mode: GizmoMode::None,
            pivot: Vec3::ZERO,
            basis: Quat::IDENTITY,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            dragging: false,
            drag_started: false,
            drag_ended: false,
        }
    }
}

impl GizmoTarget {
    /// Is a plugin currently driving the gizmo?
    pub fn engaged(&self) -> bool {
        !matches!(self.mode, GizmoMode::None | GizmoMode::Select)
    }

    /// Point the gizmo at somewhere, in a mode.
    ///
    /// Call every frame the plugin wants the handles: the gizmo reads this and
    /// nothing else, so a plugin that stops calling it hands the gizmo straight
    /// back. That is deliberate — a plugin that panics, is disabled, or simply
    /// forgets cannot strand the editor with handles nothing owns.
    pub fn engage(&mut self, mode: GizmoMode, pivot: Vec3, basis: Quat) {
        // Moving the pivot mid-drag would teleport the handles out from under
        // the cursor, so the plugin's own (possibly stale) pivot is ignored
        // until the drag finishes.
        if !self.dragging {
            self.pivot = pivot;
            self.basis = basis;
        }
        self.mode = mode;
    }

    /// Hand the gizmo back to the entity selection.
    pub fn release(&mut self) {
        *self = Self::default();
    }

    /// Clear the one-frame edges. Called by the gizmo at the end of its own
    /// frame, so a plugin reading them in `Update` sees them regardless of
    /// system order.
    pub fn clear_edges(&mut self) {
        self.drag_started = false;
        self.drag_ended = false;
    }
}
