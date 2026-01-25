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
    /// Anchor entity for Shift+click range selection
    pub selection_anchor: Option<Entity>,
}

impl SelectionState {
    /// Select a single entity, clearing multi-selection
    pub fn select(&mut self, entity: Entity) {
        self.selected_entity = Some(entity);
        self.selection_anchor = Some(entity);
        self.multi_selection.clear();
        self.multi_selection.insert(entity);
    }

    /// Clear all selection
    pub fn clear(&mut self) {
        self.selected_entity = None;
        self.selection_anchor = None;
        self.multi_selection.clear();
    }

    /// Toggle entity in multi-selection (Ctrl+click)
    pub fn toggle_selection(&mut self, entity: Entity) {
        if self.multi_selection.contains(&entity) {
            self.multi_selection.remove(&entity);
            if self.selected_entity == Some(entity) {
                self.selected_entity = self.multi_selection.iter().next().copied();
            }
        } else {
            self.multi_selection.insert(entity);
            if self.selected_entity.is_none() {
                self.selected_entity = Some(entity);
            }
            // Update anchor to the newly toggled entity
            self.selection_anchor = Some(entity);
        }
    }

    /// Select a range of entities (Shift+click)
    /// `visible_order` should contain entities in the order they appear in the hierarchy
    pub fn select_range(&mut self, target: Entity, visible_order: &[Entity]) {
        let anchor = self.selection_anchor.unwrap_or(target);

        // Find positions of anchor and target
        let anchor_pos = visible_order.iter().position(|&e| e == anchor);
        let target_pos = visible_order.iter().position(|&e| e == target);

        if let (Some(a), Some(t)) = (anchor_pos, target_pos) {
            let (start, end) = if a < t { (a, t) } else { (t, a) };

            // Clear previous selection and select range
            self.multi_selection.clear();
            for i in start..=end {
                self.multi_selection.insert(visible_order[i]);
            }

            // Update primary selection to target
            self.selected_entity = Some(target);
            // Keep the anchor where it was
        } else {
            // Fallback: just select the target
            self.select(target);
        }
    }

    /// Get all selected entities
    pub fn get_all_selected(&self) -> Vec<Entity> {
        self.multi_selection.iter().copied().collect()
    }

    /// Check if multiple entities are selected
    pub fn has_multi_selection(&self) -> bool {
        self.multi_selection.len() > 1
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
