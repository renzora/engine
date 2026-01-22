use bevy::prelude::*;
use std::collections::HashSet;

/// Tracks entity selection state in the editor
#[derive(Resource, Default)]
pub struct SelectionState {
    /// Currently selected entity (single selection)
    pub selected_entity: Option<Entity>,
    /// Multi-selection support for future use
    pub multi_selection: HashSet<Entity>,
    /// Entity for context menu (right-click)
    pub context_menu_entity: Option<Entity>,
}

impl SelectionState {
    /// Select a single entity, clearing multi-selection
    pub fn select(&mut self, entity: Entity) {
        self.selected_entity = Some(entity);
        self.multi_selection.clear();
        self.multi_selection.insert(entity);
    }

    /// Clear all selection
    pub fn clear(&mut self) {
        self.selected_entity = None;
        self.multi_selection.clear();
    }

    /// Check if an entity is selected
    pub fn is_selected(&self, entity: Entity) -> bool {
        self.selected_entity == Some(entity) || self.multi_selection.contains(&entity)
    }

    /// Add entity to multi-selection
    pub fn add_to_selection(&mut self, entity: Entity) {
        self.multi_selection.insert(entity);
        if self.selected_entity.is_none() {
            self.selected_entity = Some(entity);
        }
    }

    /// Remove entity from selection
    pub fn remove_from_selection(&mut self, entity: Entity) {
        self.multi_selection.remove(&entity);
        if self.selected_entity == Some(entity) {
            self.selected_entity = self.multi_selection.iter().next().copied();
        }
    }
}
