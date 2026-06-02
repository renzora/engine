//! Marker components for the native hierarchy tree.

use bevy::prelude::*;

/// On the list container: the cache version + expanded-set hash last rendered,
/// so the row list rebuilds only when the tree structure or expansion changes
/// (selection changes are applied in place, not rebuilt).
#[derive(Component)]
pub(crate) struct HierarchyView {
    pub version: u64,
    pub expanded_hash: u64,
}

/// A tree row. Carries the entity it represents plus the styling needed to
/// restore its un-selected look (the base background + label color).
#[derive(Component)]
pub(crate) struct HierRow {
    pub entity: Entity,
    pub base_bg: Color,
    pub label: Entity,
    pub label_color: Color,
}

/// The expand/collapse caret (a child of the row). Clicking it toggles the
/// entity's expansion without selecting the row.
#[derive(Component)]
pub(crate) struct HierCaret {
    pub entity: Entity,
}
