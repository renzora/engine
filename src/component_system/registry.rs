//! Component registry for storing and looking up component definitions

use super::definition::{ComponentCategory, ComponentDefinition};
use bevy::prelude::*;
use std::collections::HashMap;

/// Registry of all available component types
#[derive(Resource)]
pub struct ComponentRegistry {
    /// Components indexed by type_id
    components: HashMap<&'static str, &'static ComponentDefinition>,

    /// Components grouped by category
    by_category: HashMap<ComponentCategory, Vec<&'static ComponentDefinition>>,
}

impl ComponentRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Register a component definition
    pub fn register(&mut self, definition: &'static ComponentDefinition) {
        self.components.insert(definition.type_id, definition);

        self.by_category
            .entry(definition.category)
            .or_insert_with(Vec::new)
            .push(definition);

        // Sort by priority after insertion
        if let Some(list) = self.by_category.get_mut(&definition.category) {
            list.sort_by_key(|d| d.priority);
        }
    }

    /// Get a component definition by type_id
    pub fn get(&self, type_id: &str) -> Option<&'static ComponentDefinition> {
        self.components.get(type_id).copied()
    }

    /// Get all component definitions in a category
    pub fn get_by_category(&self, category: ComponentCategory) -> &[&'static ComponentDefinition] {
        self.by_category
            .get(&category)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all categories that have registered components
    pub fn categories_with_components(&self) -> impl Iterator<Item = ComponentCategory> + '_ {
        ComponentCategory::all_in_order()
            .iter()
            .copied()
            .filter(|cat| self.by_category.get(cat).map(|v| !v.is_empty()).unwrap_or(false))
    }

    /// Get all registered component definitions
    pub fn all(&self) -> impl Iterator<Item = &'static ComponentDefinition> + '_ {
        self.components.values().copied()
    }

    /// Get all components that are present on an entity
    pub fn get_present_on(&self, world: &World, entity: Entity) -> Vec<&'static ComponentDefinition> {
        self.components
            .values()
            .copied()
            .filter(|def| (def.has_fn)(world, entity))
            .collect()
    }

    /// Get all components that can be added to an entity (not already present, no conflicts)
    pub fn get_available_for(
        &self,
        world: &World,
        entity: Entity,
    ) -> Vec<&'static ComponentDefinition> {
        let present: Vec<&str> = self
            .components
            .values()
            .filter(|def| (def.has_fn)(world, entity))
            .map(|def| def.type_id)
            .collect();

        self.components
            .values()
            .copied()
            .filter(|def| {
                // Not already present
                !present.contains(&def.type_id)
                    // No conflicts with present components
                    && !present.iter().any(|p| def.conflicts_with.contains(p))
                    // Not conflicted by present components
                    && !present.iter().any(|p| {
                        self.get(p)
                            .map(|d| d.conflicts_with.contains(&def.type_id))
                            .unwrap_or(false)
                    })
            })
            .collect()
    }

    /// Check if a component can be added to an entity
    pub fn can_add(&self, world: &World, entity: Entity, type_id: &str) -> bool {
        let Some(def) = self.get(type_id) else {
            return false;
        };

        // Check not already present
        if (def.has_fn)(world, entity) {
            return false;
        }

        // Check for conflicts
        for other_def in self.components.values() {
            if (other_def.has_fn)(world, entity) {
                if def.conflicts_with.contains(&other_def.type_id)
                    || other_def.conflicts_with.contains(&def.type_id)
                {
                    return false;
                }
            }
        }

        true
    }

    /// Get the number of registered components
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
