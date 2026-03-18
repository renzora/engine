use std::collections::HashMap;

use bevy::prelude::*;

use crate::expr::Dependency;
use crate::attribute_id::AttributeId;

/// A node in the dependency graph: an (Entity, AttributeId) pair.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DepNode {
    pub entity: Entity,
    pub attribute: AttributeId,
}

impl DepNode {
    pub fn new(entity: Entity, attribute: AttributeId) -> Self {
        Self { entity, attribute }
    }
}

/// Tracks which attributes on an entity use a particular alias in their expressions.
/// When an alias is re-pointed, we use this to know which attributes need rewiring.
#[derive(Clone, Debug, Default)]
struct AliasUsage {
    /// For each attribute on the entity that references this alias,
    /// which source attributes does it depend on via this alias?
    /// Key: dependent attribute on the entity, Value: source attributes referenced via this alias.
    attribute_deps: HashMap<AttributeId, Vec<AttributeId>>,
}

/// Global dependency graph tracking all attribute-to-attribute edges and cross-entity aliases.
///
/// This is a Bevy Resource. It tracks:
/// - **Dependency edges**: both local (within one entity) and cross-entity.
/// - **Aliases**: which entity an alias on a given entity points to.
/// - **Alias usage**: which attributes on an entity reference which aliases
///   (so we can rewire edges when an alias changes).
///
/// When a attribute changes, dependents are found via this graph and re-evaluated.
/// When an alias is re-pointed, edges are automatically rewired.
#[derive(Resource, Default, Debug)]
pub struct DependencyGraph {
    /// Forward edges: when `source` changes, re-evaluate all `dependents`.
    forward: HashMap<DepNode, Vec<DepNode>>,
    /// Reverse edges: for efficient cleanup when removing a dependent.
    reverse: HashMap<DepNode, Vec<DepNode>>,
    /// Alias registry: (entity, alias_id) -> source_entity.
    aliases: HashMap<(Entity, AttributeId), Entity>,
    /// Alias usage: (entity, alias_id) -> which local attributes depend on which
    /// source attributes via this alias.
    alias_usage: HashMap<(Entity, AttributeId), AliasUsage>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    // -----------------------------------------------------------------------
    // Edge operations
    // -----------------------------------------------------------------------

    /// Register a dependency edge: `dependent` depends on `source`.
    pub fn add_edge(&mut self, source: DepNode, dependent: DepNode) {
        let fwd = self.forward.entry(source).or_default();
        if !fwd.contains(&dependent) {
            fwd.push(dependent);
        }
        let rev = self.reverse.entry(dependent).or_default();
        if !rev.contains(&source) {
            rev.push(source);
        }
    }

    /// Remove a specific dependency edge.
    pub fn remove_edge(&mut self, source: DepNode, dependent: DepNode) {
        if let Some(fwd) = self.forward.get_mut(&source) {
            fwd.retain(|d| d != &dependent);
            if fwd.is_empty() {
                self.forward.remove(&source);
            }
        }
        if let Some(rev) = self.reverse.get_mut(&dependent) {
            rev.retain(|s| s != &source);
            if rev.is_empty() {
                self.reverse.remove(&dependent);
            }
        }
    }

    /// Get all dependents of a source node.
    pub fn dependents(&self, source: DepNode) -> &[DepNode] {
        self.forward
            .get(&source)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all sources that a dependent node depends on.
    pub fn sources_of(&self, dependent: DepNode) -> &[DepNode] {
        self.reverse
            .get(&dependent)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Remove all edges where a specific (entity, attribute) is a dependent.
    pub fn remove_dependent(&mut self, dependent: DepNode) {
        if let Some(sources) = self.reverse.remove(&dependent) {
            for src in sources {
                if let Some(fwd) = self.forward.get_mut(&src) {
                    fwd.retain(|d| d != &dependent);
                    if fwd.is_empty() {
                        self.forward.remove(&src);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Alias operations
    // -----------------------------------------------------------------------

    /// Look up which entity an alias on a given entity points to.
    pub fn resolve_alias(&self, entity: Entity, alias: AttributeId) -> Option<Entity> {
        self.aliases.get(&(entity, alias)).copied()
    }

    /// Register or re-point a cross-entity source alias.
    ///
    /// Returns the list of local attributes on `entity` that need re-evaluation
    /// because their source entity changed.
    ///
    /// If an old source existed, edges from the old source are removed
    /// and edges to the new source are added.
    pub fn set_alias(
        &mut self,
        entity: Entity,
        alias: AttributeId,
        new_source: Entity,
    ) -> Vec<AttributeId> {
        let key = (entity, alias);
        let old_source = self.aliases.insert(key, new_source);

        // If source didn't change, nothing to rewire
        if old_source == Some(new_source) {
            return Vec::new();
        }

        // Get the usage info: which attributes on `entity` depend on which source
        // attributes via this alias
        let usage = match self.alias_usage.get(&key) {
            Some(u) => u.clone(),
            None => return Vec::new(),
        };

        let mut affected_attributes = Vec::new();

        for (local_attribute, source_attributes) in &usage.attribute_deps {
            let dependent = DepNode::new(entity, *local_attribute);

            for source_attribute in source_attributes {
                // Remove old edge
                if let Some(old_src) = old_source {
                    let old_node = DepNode::new(old_src, *source_attribute);
                    self.remove_edge(old_node, dependent);
                }

                // Add new edge
                let new_node = DepNode::new(new_source, *source_attribute);
                self.add_edge(new_node, dependent);
            }

            if !affected_attributes.contains(local_attribute) {
                affected_attributes.push(*local_attribute);
            }
        }

        affected_attributes
    }

    /// Remove an alias and all its associated edges.
    ///
    /// Returns the list of local attributes that need re-evaluation.
    pub fn remove_alias(&mut self, entity: Entity, alias: AttributeId) -> Vec<AttributeId> {
        let key = (entity, alias);
        let old_source = self.aliases.remove(&key);

        let usage = match self.alias_usage.remove(&key) {
            Some(u) => u,
            None => return Vec::new(),
        };

        let mut affected_attributes = Vec::new();

        if let Some(old_src) = old_source {
            for (local_attribute, source_attributes) in &usage.attribute_deps {
                let dependent = DepNode::new(entity, *local_attribute);
                for source_attribute in source_attributes {
                    let old_node = DepNode::new(old_src, *source_attribute);
                    self.remove_edge(old_node, dependent);
                }
                if !affected_attributes.contains(local_attribute) {
                    affected_attributes.push(*local_attribute);
                }
            }
        }

        affected_attributes
    }

    /// Record that a attribute on an entity uses a particular alias to reference
    /// specific source attributes. Called when an expression modifier is added.
    pub fn record_alias_usage(
        &mut self,
        entity: Entity,
        alias: AttributeId,
        local_attribute: AttributeId,
        source_attribute: AttributeId,
    ) {
        let usage = self
            .alias_usage
            .entry((entity, alias))
            .or_default();
        let deps = usage.attribute_deps.entry(local_attribute).or_default();
        if !deps.contains(&source_attribute) {
            deps.push(source_attribute);
        }
    }

    /// Remove a usage record. Called when an expression modifier is removed.
    pub fn remove_alias_usage(
        &mut self,
        entity: Entity,
        alias: AttributeId,
        local_attribute: AttributeId,
        source_attribute: AttributeId,
    ) {
        let key = (entity, alias);
        if let Some(usage) = self.alias_usage.get_mut(&key) {
            if let Some(deps) = usage.attribute_deps.get_mut(&local_attribute) {
                deps.retain(|s| s != &source_attribute);
                if deps.is_empty() {
                    usage.attribute_deps.remove(&local_attribute);
                }
            }
            if usage.attribute_deps.is_empty() {
                self.alias_usage.remove(&key);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Entity cleanup
    // -----------------------------------------------------------------------

    /// Remove ALL data involving an entity: edges, aliases, alias usage.
    /// Called when an entity is despawned.
    pub fn remove_entity(&mut self, entity: Entity) {
        // Remove forward edges where this entity is the source
        let forward_keys: Vec<DepNode> = self
            .forward
            .keys()
            .filter(|k| k.entity == entity)
            .copied()
            .collect();

        for source in &forward_keys {
            if let Some(dependents) = self.forward.remove(source) {
                for dep in dependents {
                    if let Some(rev) = self.reverse.get_mut(&dep) {
                        rev.retain(|s| s != source);
                        if rev.is_empty() {
                            self.reverse.remove(&dep);
                        }
                    }
                }
            }
        }

        // Remove reverse edges where this entity is the dependent
        let reverse_keys: Vec<DepNode> = self
            .reverse
            .keys()
            .filter(|k| k.entity == entity)
            .copied()
            .collect();

        for dependent in &reverse_keys {
            if let Some(sources) = self.reverse.remove(dependent) {
                for src in sources {
                    if let Some(fwd) = self.forward.get_mut(&src) {
                        fwd.retain(|d| d != dependent);
                        if fwd.is_empty() {
                            self.forward.remove(&src);
                        }
                    }
                }
            }
        }

        // Remove aliases owned by this entity
        let alias_keys: Vec<(Entity, AttributeId)> = self
            .aliases
            .keys()
            .filter(|(e, _)| *e == entity)
            .copied()
            .collect();
        for key in alias_keys {
            self.aliases.remove(&key);
        }

        // Remove alias usage for this entity
        let usage_keys: Vec<(Entity, AttributeId)> = self
            .alias_usage
            .keys()
            .filter(|(e, _)| *e == entity)
            .copied()
            .collect();
        for key in usage_keys {
            self.alias_usage.remove(&key);
        }
    }

    /// Check if the graph has any edges.
    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    /// Check if the graph has any aliases.
    pub fn has_aliases(&self) -> bool {
        !self.aliases.is_empty()
    }
}

/// Helper: register dependency edges for an expression's dependencies.
/// This is used by `AttributesMut` when adding expression modifiers.
pub fn register_expr_deps(
    graph: &mut DependencyGraph,
    entity: Entity,
    attribute_id: AttributeId,
    deps: &[Dependency],
) {
    let dependent = DepNode::new(entity, attribute_id);

    for dep in deps {
        match dep {
            Dependency::Local(source_attribute) => {
                let source = DepNode::new(entity, *source_attribute);
                graph.add_edge(source, dependent);
            }
            Dependency::Source { alias, attribute }
            | Dependency::SourceTagQuery { alias, attribute, .. } => {
                graph.record_alias_usage(entity, *alias, attribute_id, *attribute);

                if let Some(source_entity) = graph.resolve_alias(entity, *alias) {
                    let source = DepNode::new(source_entity, *attribute);
                    graph.add_edge(source, dependent);
                }
            }
            Dependency::TagQuery { synthetic, .. } => {
                let source = DepNode::new(entity, *synthetic);
                graph.add_edge(source, dependent);
            }
        }
    }
}

/// Helper: unregister dependency edges for an expression's dependencies.
pub fn unregister_expr_deps(
    graph: &mut DependencyGraph,
    entity: Entity,
    attribute_id: AttributeId,
    deps: &[Dependency],
) {
    let dependent = DepNode::new(entity, attribute_id);

    for dep in deps {
        match dep {
            Dependency::Local(source_attribute) => {
                let source = DepNode::new(entity, *source_attribute);
                graph.remove_edge(source, dependent);
            }
            Dependency::Source { alias, attribute }
            | Dependency::SourceTagQuery { alias, attribute, .. } => {
                graph.remove_alias_usage(entity, *alias, attribute_id, *attribute);

                if let Some(source_entity) = graph.resolve_alias(entity, *alias) {
                    let source = DepNode::new(source_entity, *attribute);
                    graph.remove_edge(source, dependent);
                }
            }
            Dependency::TagQuery { synthetic, .. } => {
                let source = DepNode::new(entity, *synthetic);
                graph.remove_edge(source, dependent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute_id::Interner;

    fn make_entity(id: u32) -> Entity {
        Entity::from_raw_u32(id).expect("test entity")
    }

    #[test]
    fn add_and_query_edge() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let e = make_entity(1);
        let strength = interner.get_or_intern("Strength");
        let health = interner.get_or_intern("Health");

        let source = DepNode::new(e, strength);
        let dependent = DepNode::new(e, health);

        graph.add_edge(source, dependent);
        assert_eq!(graph.dependents(source), &[dependent]);
        assert_eq!(graph.sources_of(dependent), &[source]);
    }

    #[test]
    fn remove_edge() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let e = make_entity(1);
        let a = interner.get_or_intern("A");
        let b = interner.get_or_intern("B");

        let source = DepNode::new(e, a);
        let dependent = DepNode::new(e, b);

        graph.add_edge(source, dependent);
        graph.remove_edge(source, dependent);
        assert!(graph.dependents(source).is_empty());
        assert!(graph.sources_of(dependent).is_empty());
        assert!(graph.is_empty());
    }

    #[test]
    fn remove_entity_cleans_all_edges() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let e1 = make_entity(1);
        let e2 = make_entity(2);
        let a = interner.get_or_intern("A");
        let b = interner.get_or_intern("B");

        graph.add_edge(DepNode::new(e1, a), DepNode::new(e2, b));
        graph.add_edge(DepNode::new(e2, a), DepNode::new(e2, b));

        graph.remove_entity(e2);
        assert!(graph.dependents(DepNode::new(e1, a)).is_empty());
        assert!(graph.is_empty());
    }

    #[test]
    fn no_duplicate_edges() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let e = make_entity(1);
        let a = interner.get_or_intern("A");
        let b = interner.get_or_intern("B");

        let source = DepNode::new(e, a);
        let dependent = DepNode::new(e, b);

        graph.add_edge(source, dependent);
        graph.add_edge(source, dependent);
        assert_eq!(graph.dependents(source).len(), 1);
    }

    #[test]
    fn alias_set_and_resolve() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player = make_entity(2);
        let wielder = interner.get_or_intern("Wielder");

        graph.set_alias(sword, wielder, player);
        assert_eq!(graph.resolve_alias(sword, wielder), Some(player));
    }

    #[test]
    fn alias_rewire_on_change() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player_a = make_entity(2);
        let player_b = make_entity(3);
        let wielder = interner.get_or_intern("Wielder");
        let strength = interner.get_or_intern("Strength");
        let attack = interner.get_or_intern("AttackPower");

        // Sword's AttackPower depends on Wielder's Strength
        graph.record_alias_usage(sword, wielder, attack, strength);

        // Point alias to player_a and add edge
        graph.set_alias(sword, wielder, player_a);
        // set_alias wired: (player_a, Strength) -> (sword, AttackPower)
        assert_eq!(
            graph.dependents(DepNode::new(player_a, strength)),
            &[DepNode::new(sword, attack)]
        );

        // Re-point to player_b — should rewire
        let affected = graph.set_alias(sword, wielder, player_b);
        assert!(affected.contains(&attack));
        // Old edge gone
        assert!(graph.dependents(DepNode::new(player_a, strength)).is_empty());
        // New edge present
        assert_eq!(
            graph.dependents(DepNode::new(player_b, strength)),
            &[DepNode::new(sword, attack)]
        );
    }

    #[test]
    fn alias_remove_cleans_edges() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player = make_entity(2);
        let wielder = interner.get_or_intern("Wielder");
        let strength = interner.get_or_intern("Strength");
        let attack = interner.get_or_intern("AttackPower");

        graph.record_alias_usage(sword, wielder, attack, strength);
        graph.set_alias(sword, wielder, player);

        let affected = graph.remove_alias(sword, wielder);
        assert!(affected.contains(&attack));
        assert!(graph.dependents(DepNode::new(player, strength)).is_empty());
        assert!(graph.resolve_alias(sword, wielder).is_none());
    }

    #[test]
    fn remove_entity_cleans_aliases() {
        let interner = Interner::new();
        let mut graph = DependencyGraph::new();
        let sword = make_entity(1);
        let player = make_entity(2);
        let wielder = interner.get_or_intern("Wielder");

        graph.set_alias(sword, wielder, player);
        graph.remove_entity(sword);
        assert!(graph.resolve_alias(sword, wielder).is_none());
        assert!(!graph.has_aliases());
    }
}
