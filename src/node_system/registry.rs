use bevy::prelude::*;
use std::collections::HashMap;

use super::definition::{NodeCategory, NodeDefinition};

/// Central registry of all node types
/// This is a Bevy Resource that stores references to all registered NodeDefinitions
#[derive(Resource, Default)]
pub struct NodeRegistry {
    /// All registered node definitions, keyed by type_id
    definitions: HashMap<&'static str, &'static NodeDefinition>,
    /// Node definitions grouped by category for menu rendering
    by_category: HashMap<NodeCategory, Vec<&'static NodeDefinition>>,
}

impl NodeRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Register a node definition
    pub fn register(&mut self, definition: &'static NodeDefinition) {
        // Add to main lookup
        self.definitions.insert(definition.type_id, definition);

        // Add to category lookup
        self.by_category
            .entry(definition.category)
            .or_insert_with(Vec::new)
            .push(definition);

        // Sort the category list by priority
        if let Some(defs) = self.by_category.get_mut(&definition.category) {
            defs.sort_by_key(|d| d.priority);
        }
    }

    /// Get a node definition by type_id
    pub fn get(&self, type_id: &str) -> Option<&'static NodeDefinition> {
        self.definitions.get(type_id).copied()
    }

    /// Get all node definitions in a category, sorted by priority
    pub fn get_by_category(&self, category: NodeCategory) -> &[&'static NodeDefinition] {
        self.by_category
            .get(&category)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all categories that have registered nodes, in menu order
    pub fn categories_with_nodes(&self) -> Vec<NodeCategory> {
        let mut categories: Vec<_> = self
            .by_category
            .keys()
            .filter(|cat| !self.by_category.get(*cat).unwrap().is_empty())
            .copied()
            .collect();
        categories.sort_by_key(|c| c.menu_order());
        categories
    }

    /// Get all registered node definitions (kept for future use)
    #[allow(dead_code)]
    pub fn all_definitions(&self) -> impl Iterator<Item = &'static NodeDefinition> + '_ {
        self.definitions.values().copied()
    }

    /// Check if a type_id is registered (kept for future use)
    #[allow(dead_code)]
    pub fn contains(&self, type_id: &str) -> bool {
        self.definitions.contains_key(type_id)
    }

    /// Get the number of registered node types
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    /// Check if the registry is empty (kept for future use)
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}
