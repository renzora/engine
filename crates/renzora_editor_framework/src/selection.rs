//! Global editor selection — shared between hierarchy, inspector, and viewport.

use std::sync::RwLock;

use bevy::prelude::*;

/// Global editor selection resource.
///
/// Uses `RwLock` so panels (which receive `&World`) can read selection,
/// while the hierarchy (also `&World`) can write via interior mutability.
#[derive(Resource)]
pub struct EditorSelection {
    selected: RwLock<Vec<Entity>>,
}

impl Default for EditorSelection {
    fn default() -> Self {
        Self {
            selected: RwLock::new(Vec::new()),
        }
    }
}

impl EditorSelection {
    /// Get the primary selected entity (first in list), for backward compat.
    pub fn get(&self) -> Option<Entity> {
        self.selected.read().unwrap().first().copied()
    }

    /// Get all selected entities.
    pub fn get_all(&self) -> Vec<Entity> {
        self.selected.read().unwrap().clone()
    }

    /// Set a single selected entity (clears previous selection).
    pub fn set(&self, entity: Option<Entity>) {
        let mut sel = self.selected.write().unwrap();
        sel.clear();
        if let Some(e) = entity {
            sel.push(e);
            renzora::console_log::console_info("Selection", &format!("Selected {:?}", e));
        } else {
            renzora::console_log::console_info("Selection", "Selection cleared");
        }
    }

    /// Set multiple selected entities.
    pub fn set_multiple(&self, entities: Vec<Entity>) {
        renzora::console_log::console_info(
            "Selection",
            &format!("Multi-select: {} entities {:?}", entities.len(), entities),
        );
        *self.selected.write().unwrap() = entities;
    }

    /// Toggle an entity in the selection (add if absent, remove if present).
    pub fn toggle(&self, entity: Entity) {
        let mut sel = self.selected.write().unwrap();
        if let Some(pos) = sel.iter().position(|&e| e == entity) {
            sel.remove(pos);
            renzora::console_log::console_info("Selection", &format!("Deselected {:?}", entity));
        } else {
            sel.push(entity);
            renzora::console_log::console_info("Selection", &format!("Added {:?} to selection", entity));
        }
    }

    /// Check if an entity is currently selected.
    pub fn is_selected(&self, entity: Entity) -> bool {
        self.selected.read().unwrap().contains(&entity)
    }

    /// Select a range of entities from the visible order list.
    /// Selects all entities between `anchor` and `target` (inclusive) in the given order.
    pub fn select_range(&self, visible_order: &[Entity], anchor: Entity, target: Entity) {
        let anchor_idx = visible_order.iter().position(|&e| e == anchor);
        let target_idx = visible_order.iter().position(|&e| e == target);
        if let (Some(a), Some(b)) = (anchor_idx, target_idx) {
            let (start, end) = if a <= b { (a, b) } else { (b, a) };
            let range: Vec<Entity> = visible_order[start..=end].to_vec();
            *self.selected.write().unwrap() = range;
        }
    }

    /// Whether more than one entity is selected.
    pub fn has_multi_selection(&self) -> bool {
        self.selected.read().unwrap().len() > 1
    }

    /// Clear the selection.
    pub fn clear(&self) {
        self.selected.write().unwrap().clear();
    }
}
