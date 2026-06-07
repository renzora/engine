//! Bevy-only editor data types shared across the binary↔bundle boundary.
//! Moved from `renzora_editor`'s lib.rs in the Operation Merge contract fold —
//! `renzora_game_ui` (a binary-linked dual-mode crate) references these, so
//! they must live in the shared `renzora` dylib for one `TypeId`.

use bevy::prelude::*;

/// Sort order for root-level entities in the hierarchy panel.
/// Lower values appear first. Entities without this component sort last.
#[derive(Component, Clone, Copy)]
pub struct HierarchyOrder(pub u32);

/// Pending entities to expand in the hierarchy panel next time it renders.
/// Systems that spawn entities as children can push the parent entity here so
/// the panel reveals the newly spawned child even if the user hasn't toggled
/// expansion manually.
#[derive(Resource, Default)]
pub struct HierarchyExpandRequests {
    entries: std::sync::RwLock<Vec<Entity>>,
}

impl HierarchyExpandRequests {
    pub fn push(&self, entity: Entity) {
        self.entries.write().unwrap().push(entity);
    }
    pub fn drain(&self) -> Vec<Entity> {
        std::mem::take(&mut *self.entries.write().unwrap())
    }
}
