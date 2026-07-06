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

/// Which authored asset an [`HierAssetBadge`] points at — selects the editor a
/// click opens (code editor / blueprint graph / material graph).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeKind {
    Script,
    Blueprint,
    Material,
}

/// A clickable asset badge (script / blueprint / material) at a row's right
/// edge, just left of the eye/lock toggles. Clicking it opens that asset in its
/// editor for the badge's entity.
#[derive(Component)]
pub(crate) struct HierAssetBadge {
    pub entity: Entity,
    pub kind: BadgeKind,
}

/// The expand/collapse caret (only present on rows with children). Clicking it
/// toggles the row's expansion; the rest of the row selects.
#[derive(Component)]
pub(crate) struct HierCaretToggle(pub Entity);

/// A sticky "parent stack" header row (the ancestor that pins to the top while
/// you scroll a deep tree). Clicking it collapses that branch, keeps it selected,
/// and scrolls the tree back to its real row. See [`super::pin`].
#[derive(Component)]
pub(crate) struct HierPinClick {
    pub entity: Entity,
}

/// A drop-indicator line at a row's top (`after = false`) or bottom
/// (`after = true`) edge, shown during a drag when this row is the
/// Before/After target. Hidden otherwise.
#[derive(Component)]
pub(crate) struct HierDropEdge {
    pub entity: Entity,
    pub after: bool,
}
