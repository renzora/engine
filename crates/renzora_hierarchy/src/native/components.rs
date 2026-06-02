//! Marker components for the native hierarchy tree.

use bevy::prelude::*;

/// On the list container: the cache version + expanded-set hash last rendered,
/// so the row list rebuilds only when the tree structure or expansion changes
/// (selection changes are applied in place, not rebuilt).
#[derive(Component)]
pub(crate) struct HierarchyView {
    /// Hash of the visible tree content (entities/names/icons/flags/depth/
    /// expansion — NOT selection). Rebuild rows only when this changes, so the
    /// hierarchy cache being dirtied every frame (with identical content)
    /// doesn't thrash the row list.
    pub content_hash: u64,
}

/// The full-row background layer (visual only). Carries the entity it shows plus
/// the styling needed to restore its un-selected look, and a handle to the click
/// layer so the visual system can read hover state.
#[derive(Component)]
pub(crate) struct HierRow {
    pub entity: Entity,
    pub base_bg: Color,
    pub label: Entity,
    pub label_color: Color,
    pub click: Entity,
}

/// The row's click target — covers the row *except* the right-edge suffix zone
/// (eye/lock), so clicking a toggle never selects/expands the row.
#[derive(Component)]
pub(crate) struct HierRowClick {
    pub entity: Entity,
    pub has_children: bool,
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
