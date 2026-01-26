//! Dependency resolution for plugins.
//!
//! This module provides topological sorting of plugins based on their dependencies.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use super::abi::{PluginError, PluginManifest};

/// Dependency graph for plugin load ordering
pub struct DependencyGraph {
    nodes: HashSet<String>,
    edges: HashMap<String, Vec<String>>, // plugin -> depends on
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            edges: HashMap::new(),
        }
    }

    /// Add a plugin node to the graph
    pub fn add_node(&mut self, plugin_id: &str) {
        self.nodes.insert(plugin_id.to_string());
    }

    /// Add a dependency edge (from depends on to)
    pub fn add_edge(&mut self, from: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_default()
            .push(to.to_string());
    }

    /// Build graph from plugin manifests
    pub fn from_manifests(manifests: &[PluginManifest]) -> Self {
        let mut graph = Self::new();

        for manifest in manifests {
            graph.add_node(&manifest.id);
            for dep in &manifest.dependencies {
                if !dep.optional {
                    graph.add_edge(&manifest.id, &dep.plugin_id);
                }
            }
        }

        graph
    }

    /// Perform topological sort to determine load order.
    /// Returns plugin IDs in the order they should be loaded (dependencies first).
    pub fn topological_sort(&self) -> Result<Vec<String>, PluginError> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        for node in &self.nodes {
            if !visited.contains(node) {
                self.visit(node, &mut visited, &mut temp_visited, &mut result)?;
            }
        }

        Ok(result)
    }

    fn visit(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        temp_visited: &mut HashSet<String>,
        result: &mut Vec<String>,
    ) -> Result<(), PluginError> {
        if temp_visited.contains(node) {
            return Err(PluginError::CircularDependency(node.to_string()));
        }

        if visited.contains(node) {
            return Ok(());
        }

        temp_visited.insert(node.to_string());

        if let Some(deps) = self.edges.get(node) {
            for dep in deps {
                if !self.nodes.contains(dep) {
                    return Err(PluginError::MissingDependency {
                        plugin: node.to_string(),
                        dependency: dep.clone(),
                    });
                }
                self.visit(dep, visited, temp_visited, result)?;
            }
        }

        temp_visited.remove(node);
        visited.insert(node.to_string());
        result.push(node.to_string());

        Ok(())
    }

    /// Check if all dependencies are satisfied
    pub fn validate(&self, available_plugins: &HashSet<String>) -> Result<(), PluginError> {
        for (plugin, deps) in &self.edges {
            for dep in deps {
                if !available_plugins.contains(dep) {
                    return Err(PluginError::MissingDependency {
                        plugin: plugin.clone(),
                        dependency: dep.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_sort() {
        let mut graph = DependencyGraph::new();
        graph.add_node("a");
        graph.add_node("b");
        graph.add_node("c");
        graph.add_edge("a", "b"); // a depends on b
        graph.add_edge("b", "c"); // b depends on c

        let order = graph.topological_sort().unwrap();
        assert_eq!(order, vec!["c", "b", "a"]);
    }

    #[test]
    fn test_circular_dependency() {
        let mut graph = DependencyGraph::new();
        graph.add_node("a");
        graph.add_node("b");
        graph.add_edge("a", "b");
        graph.add_edge("b", "a"); // circular!

        let result = graph.topological_sort();
        assert!(matches!(result, Err(PluginError::CircularDependency(_))));
    }

    #[test]
    fn test_missing_dependency() {
        let mut graph = DependencyGraph::new();
        graph.add_node("a");
        graph.add_edge("a", "missing"); // missing is not a node

        let result = graph.topological_sort();
        assert!(matches!(result, Err(PluginError::MissingDependency { .. })));
    }
}
