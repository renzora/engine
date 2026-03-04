//! Global editor selection — shared between hierarchy, inspector, and viewport.

use std::sync::RwLock;

use bevy::prelude::*;

/// Global editor selection resource.
///
/// Uses `RwLock` so panels (which receive `&World`) can read selection,
/// while the hierarchy (also `&World`) can write via interior mutability.
#[derive(Resource)]
pub struct EditorSelection {
    selected: RwLock<Option<Entity>>,
}

impl Default for EditorSelection {
    fn default() -> Self {
        Self {
            selected: RwLock::new(None),
        }
    }
}

impl EditorSelection {
    /// Get the currently selected entity (if any).
    pub fn get(&self) -> Option<Entity> {
        *self.selected.read().unwrap()
    }

    /// Set the selected entity.
    pub fn set(&self, entity: Option<Entity>) {
        *self.selected.write().unwrap() = entity;
    }
}
