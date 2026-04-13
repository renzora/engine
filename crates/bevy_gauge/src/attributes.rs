use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::context::AttributeContext;
use crate::node::{ReduceFn, AttributeNode};
use crate::attribute_id::{global_rodeo, AttributeId};
use crate::tags::TagMask;

/// Template for lazy materialization of tagged complex attributes.
///
/// Stored on [`Attributes`] when a attribute is created via
/// [`tagged_attribute`](crate::attributes_mut::AttributesMut::tagged_attribute).
/// When `evaluate_tagged` is called for a tag combo that hasn't been seen yet,
/// the template is used to auto-generate a tagged expression modifier.
#[derive(Clone, Debug)]
pub(crate) struct AttributeTemplate {
    /// The expression with short part names (e.g., `"Added * (1 + Increased)"`).
    pub expression: String,
    /// Part names used in the expression (e.g., `["Added", "Increased"]`).
    pub parts: Vec<String>,
    /// Parent attribute name used as qualifier prefix (e.g., `"Damage"`).
    pub name: String,
    /// Tag combos that have already been materialized as expression modifiers.
    pub materialized: HashSet<TagMask>,
}

/// The per-entity attribute storage component.
///
/// Holds all attribute nodes and their current evaluated values.
/// Read access requires only `&Attributes` — no special system params needed.
///
/// Writes (adding/removing modifiers, setting values) go through
/// `AttributesMut` to ensure dependency propagation.
///
/// ## Tag Queries
///
/// Tagged attribute queries (e.g. "all FIRE damage") are **materialized as
/// synthetic attribute nodes** in the dependency graph. When a tag query is first
/// requested, a synthetic `AttributeId` is created and wired as a dependent of
/// the parent attribute. From then on, changes to the parent automatically
/// propagate to and re-evaluate the tag query — no separate cache needed.
///
/// Use [`get_tagged`](Self::get_tagged) to read cached tag-query results.
/// The query must have been registered first via `AttributesMut::evaluate_tagged`
/// or by compiling an expression that contains `{TAG}` syntax.
#[derive(Component, Clone, Debug, Default)]
pub struct Attributes {
    /// Attribute nodes keyed by AttributeId.
    pub(crate) nodes: HashMap<AttributeId, AttributeNode>,
    /// Current evaluated values. This is the evaluation context
    /// that expressions read from. Also holds cached source attribute values
    /// under composite keys like `AttributeName@Alias`, and cached tag-query
    /// results under synthetic AttributeIds.
    pub(crate) context: AttributeContext,
    /// Forward map: synthetic AttributeId → (parent attribute, tag mask).
    /// Used by `evaluate_and_cache` to know how to evaluate tag-query nodes.
    pub(crate) tag_queries: HashMap<AttributeId, (AttributeId, TagMask)>,
    /// Reverse map: (parent attribute, tag mask) → synthetic AttributeId.
    /// Used by `get_tagged` to look up the cached value.
    pub(crate) tag_query_ids: HashMap<(AttributeId, TagMask), AttributeId>,
    /// Templates for tagged attributes. When `evaluate_tagged` is called for
    /// a tag combo that hasn't been materialized yet, the template is used to
    /// auto-generate a tagged expression modifier on the fly.
    pub(crate) templates: HashMap<AttributeId, AttributeTemplate>,
}

impl Attributes {
    /// Create a new empty Attributes component.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read a attribute value by AttributeId. Returns 0.0 if the attribute doesn't exist.
    pub fn get(&self, id: AttributeId) -> f32 {
        self.context.get(id)
    }

    /// Read a attribute by string name using the global interner.
    ///
    /// Requires [`AttributesPlugin`](crate::plugin::AttributesPlugin) to have
    /// been added. Panics otherwise.
    pub fn value(&self, name: &str) -> f32 {
        if let Some(spur) = global_rodeo().get(name) {
            self.context.get(AttributeId(spur))
        } else {
            0.0
        }
    }

    /// Read a tagged attribute query by string name using the global interner.
    ///
    /// Requires [`AttributesPlugin`](crate::plugin::AttributesPlugin) to have
    /// been added. Panics otherwise.
    pub fn value_tagged(&self, name: &str, mask: TagMask) -> f32 {
        if let Some(spur) = global_rodeo().get(name) {
            self.get_tagged(AttributeId(spur), mask)
        } else {
            0.0
        }
    }

    /// Read a tagged attribute query result by AttributeId and tag mask.
    ///
    /// Returns the cached result if the tag query has been registered (via
    /// `AttributesMut::evaluate_tagged` or an expression with `{TAG}` syntax).
    /// Returns 0.0 if the query hasn't been registered yet.
    ///
    /// If `mask` is `TagMask::NONE`, delegates to [`get`](Self::get).
    pub fn get_tagged(&self, id: AttributeId, mask: TagMask) -> f32 {
        if mask.is_empty() {
            return self.context.get(id);
        }
        if let Some(&synthetic_id) = self.tag_query_ids.get(&(id, mask)) {
            self.context.get(synthetic_id)
        } else {
            0.0
        }
    }

    /// Check if a attribute node exists.
    pub fn has_attribute(&self, id: AttributeId) -> bool {
        self.nodes.contains_key(&id)
    }

    /// Iterate over all (AttributeId, current_value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (AttributeId, f32)> + '_ {
        self.context.iter()
    }

    // --- Internal mutation methods (used by AttributesMut) ---

    /// Ensure a node exists for the given attribute, creating one with the given
    /// reduce function if absent.
    pub(crate) fn ensure_node(&mut self, id: AttributeId, reduce: ReduceFn) -> &mut AttributeNode {
        self.nodes.entry(id).or_insert_with(|| AttributeNode::new(reduce))
    }

    /// Re-evaluate a attribute node and update the context. Returns the new value.
    ///
    /// If `id` is a synthetic tag-query node, evaluates the parent's modifiers
    /// filtered by the tag mask instead of looking up a node directly.
    pub(crate) fn evaluate_and_cache(&mut self, id: AttributeId) -> f32 {
        let value = if let Some(&(parent_id, mask)) = self.tag_queries.get(&id) {
            // Synthetic tag-query node: evaluate the parent's modifiers with tag filter
            if let Some(node) = self.nodes.get(&parent_id) {
                node.evaluate_tagged(&self.context, mask)
            } else {
                0.0
            }
        } else if let Some(node) = self.nodes.get(&id) {
            // Normal attribute node
            node.evaluate(&self.context)
        } else {
            0.0
        };
        self.context.set(id, value);
        value
    }

    /// Register a tag query, returning the synthetic AttributeId.
    /// If the query already exists, returns the existing synthetic ID.
    pub(crate) fn register_tag_query(
        &mut self,
        parent_id: AttributeId,
        mask: TagMask,
        synthetic_id: AttributeId,
    ) {
        self.tag_queries.insert(synthetic_id, (parent_id, mask));
        self.tag_query_ids.insert((parent_id, mask), synthetic_id);
    }

    /// Check if a tag query is already registered.
    pub(crate) fn tag_query_synthetic_id(
        &self,
        parent_id: AttributeId,
        mask: TagMask,
    ) -> Option<AttributeId> {
        self.tag_query_ids.get(&(parent_id, mask)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute_id::Interner;
    use crate::modifier::Modifier;

    #[test]
    fn empty_attributes() {
        let interner = Interner::new();
        let attrs = Attributes::new();
        let id = interner.get_or_intern("Nonexistent");
        assert_eq!(attrs.get(id), 0.0);
    }

    #[test]
    fn basic_attribute_lifecycle() {
        let interner = Interner::new();
        let mut attrs = Attributes::new();
        let id = interner.get_or_intern("Strength");

        let node = attrs.ensure_node(id, ReduceFn::Sum);
        node.add_modifier(Modifier::Flat(25.0));

        let val = attrs.evaluate_and_cache(id);
        assert_eq!(val, 25.0);
        assert_eq!(attrs.get(id), 25.0);
    }

    #[test]
    fn get_tagged_none_delegates_to_get() {
        let interner = Interner::new();
        let mut attrs = Attributes::new();
        let id = interner.get_or_intern("Damage");

        let node = attrs.ensure_node(id, ReduceFn::Sum);
        node.add_modifier(Modifier::Flat(50.0));
        attrs.evaluate_and_cache(id);

        assert_eq!(attrs.get_tagged(id, TagMask::NONE), 50.0);
    }

    #[test]
    fn get_tagged_unregistered_returns_zero() {
        let interner = Interner::new();
        let attrs = Attributes::new();
        let id = interner.get_or_intern("Damage");
        assert_eq!(attrs.get_tagged(id, TagMask::bit(0)), 0.0);
    }

    #[test]
    fn tag_query_evaluate_and_cache() {
        let interner = Interner::new();
        let fire = TagMask::bit(0);
        let physical = TagMask::bit(1);

        let mut attrs = Attributes::new();
        let damage_id = interner.get_or_intern("Damage");
        let synthetic_id = interner.get_or_intern("\0tag:Damage:1");

        // Add tagged modifiers to the damage node
        let node = attrs.ensure_node(damage_id, ReduceFn::Sum);
        node.add_tagged_modifier(Modifier::Flat(25.0), fire);
        node.add_tagged_modifier(Modifier::Flat(10.0), physical);
        node.add_modifier(Modifier::Flat(5.0)); // global

        // Register the tag query for FIRE
        attrs.register_tag_query(damage_id, fire, synthetic_id);

        // Evaluate the parent first (needed for context)
        attrs.evaluate_and_cache(damage_id);
        // Evaluate the synthetic node
        let val = attrs.evaluate_and_cache(synthetic_id);

        // FIRE query: fire modifier (25) + global (5) = 30
        assert_eq!(val, 30.0);
        assert_eq!(attrs.get_tagged(damage_id, fire), 30.0);
    }
}
