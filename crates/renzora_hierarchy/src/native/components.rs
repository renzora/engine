//! Marker components for the native hierarchy tree.

use bevy::prelude::*;

/// The row's click target — covers the row *except* the right-edge suffix zone
/// (eye/lock), so clicking a toggle never selects/expands the row.
#[derive(Component)]
pub(crate) struct HierRowClick {
    pub entity: Entity,
}

/// The eye toggle at a row's right edge. `visible` is the current state (so the
/// click handler can record `was_visible` for undo).
#[derive(Component)]
pub(crate) struct HierVisToggle {
    pub entity: Entity,
    pub visible: bool,
}

/// The lock toggle at a row's right edge.
#[derive(Component)]
pub(crate) struct HierLockToggle {
    pub entity: Entity,
    pub locked: bool,
}

/// The expand/collapse caret (only present on rows with children). Clicking it
/// toggles the row's expansion; the rest of the row selects.
#[derive(Component)]
pub(crate) struct HierCaretToggle(pub Entity);

/// A drop-indicator line at a row's top (`after = false`) or bottom
/// (`after = true`) edge, shown during a drag when this row is the
/// Before/After target. Hidden otherwise.
#[derive(Component)]
pub(crate) struct HierDropEdge {
    pub entity: Entity,
    pub after: bool,
}
