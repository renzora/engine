use crate::context::AttributeContext;
use crate::modifier::{Modifier, TaggedModifier};
use crate::tags::TagMask;

/// How a attribute node's modifiers are reduced to produce a single value.
#[derive(Clone, Debug)]
pub enum ReduceFn {
    /// Sum all modifier values. Default for "added"/"flat" style attributes.
    Sum,
    /// Multiply all modifier values. Default for "more"/"less" style multipliers.
    /// The base is 1.0; each modifier is treated as `(1 + modifier_value)`.
    Product,
    /// User-defined reduction function.
    Custom(fn(&[f32]) -> f32),
}

impl Default for ReduceFn {
    fn default() -> Self {
        ReduceFn::Sum
    }
}

/// A attribute node — the fundamental unit of the attribute graph.
///
/// Holds a collection of tagged modifiers and a reduce function that combines
/// them into a single value. Each modifier carries a [`TagMask`] indicating
/// which attribute/damage types it applies to; see [`TaggedModifier`].
#[derive(Clone, Debug)]
pub struct AttributeNode {
    /// How modifiers are combined.
    pub reduce: ReduceFn,
    /// Active tagged modifiers on this node.
    pub modifiers: Vec<TaggedModifier>,
}

impl AttributeNode {
    /// Create a new node with the given reduce function and no modifiers.
    pub fn new(reduce: ReduceFn) -> Self {
        Self {
            reduce,
            modifiers: Vec::new(),
        }
    }

    /// Create a new Sum-reducing node.
    pub fn sum() -> Self {
        Self::new(ReduceFn::Sum)
    }

    /// Create a new Product-reducing node.
    pub fn product() -> Self {
        Self::new(ReduceFn::Product)
    }

    /// Add a modifier to this node (untagged — applies to every tag query).
    pub fn add_modifier(&mut self, modifier: Modifier) {
        self.modifiers.push(TaggedModifier::global(modifier));
    }

    /// Add a tagged modifier to this node.
    pub fn add_tagged_modifier(&mut self, modifier: Modifier, tag: TagMask) {
        self.modifiers.push(TaggedModifier::new(modifier, tag));
    }

    /// Remove the first modifier whose value matches (ignoring tags).
    /// Returns true if found and removed.
    pub fn remove_modifier(&mut self, modifier: &Modifier) -> bool {
        if let Some(pos) = self
            .modifiers
            .iter()
            .position(|tm| &tm.modifier == modifier)
        {
            self.modifiers.remove(pos);
            true
        } else {
            false
        }
    }

    /// Remove the first modifier that matches both value and tag.
    /// Returns true if found and removed.
    pub fn remove_tagged_modifier(&mut self, modifier: &Modifier, tag: TagMask) -> bool {
        let target = TaggedModifier::new(modifier.clone(), tag);
        if let Some(pos) = self.modifiers.iter().position(|tm| tm == &target) {
            self.modifiers.remove(pos);
            true
        } else {
            false
        }
    }

    /// Evaluate this node: evaluate **all** modifiers (ignoring tags), then reduce.
    pub fn evaluate(&self, context: &AttributeContext) -> f32 {
        let iter = self.modifiers.iter().map(|tm| tm.modifier.evaluate(context));
        self.reduce_iter(iter)
    }

    /// Evaluate only modifiers whose tags match the given query, then reduce.
    ///
    /// A modifier matches if its tag is NONE (global) or its tag bits are a
    /// subset of `query`. See [`TagMask::matches_query`].
    pub fn evaluate_tagged(&self, context: &AttributeContext, query: TagMask) -> f32 {
        let iter = self
            .modifiers
            .iter()
            .filter(|tm| tm.tag.matches_query(query))
            .map(|tm| tm.modifier.evaluate(context));
        self.reduce_iter(iter)
    }

    /// Reduce an iterator of evaluated modifier values using this node's reduce function.
    ///
    /// Sum and Product fold directly without allocating. Custom still requires
    /// collecting into a Vec because its function signature takes `&[f32]`.
    fn reduce_iter(&self, iter: impl Iterator<Item = f32>) -> f32 {
        match &self.reduce {
            ReduceFn::Sum => iter.sum(),
            ReduceFn::Product => iter.map(|v| 1.0 + v).product(),
            ReduceFn::Custom(f) => {
                let values: Vec<f32> = iter.collect();
                if values.is_empty() { 0.0 } else { f(&values) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_node() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::sum();
        node.add_modifier(Modifier::Flat(10.0));
        node.add_modifier(Modifier::Flat(5.0));
        assert_eq!(node.evaluate(&ctx), 15.0);
    }

    #[test]
    fn product_node() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::product();
        node.add_modifier(Modifier::Flat(0.5)); // 1.5x
        node.add_modifier(Modifier::Flat(0.3)); // 1.3x
        let result = node.evaluate(&ctx);
        // 1.5 * 1.3 = 1.95
        assert!((result - 1.95).abs() < 0.001);
    }

    #[test]
    fn empty_sum_is_zero() {
        let ctx = AttributeContext::new();
        let node = AttributeNode::sum();
        assert_eq!(node.evaluate(&ctx), 0.0);
    }

    #[test]
    fn empty_product_is_one() {
        let ctx = AttributeContext::new();
        let node = AttributeNode::product();
        assert_eq!(node.evaluate(&ctx), 1.0);
    }

    #[test]
    fn remove_modifier() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::sum();
        node.add_modifier(Modifier::Flat(10.0));
        node.add_modifier(Modifier::Flat(5.0));
        assert!(node.remove_modifier(&Modifier::Flat(10.0)));
        assert_eq!(node.evaluate(&ctx), 5.0);
    }

    #[test]
    fn custom_reduce() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::new(ReduceFn::Custom(|vals| {
            vals.iter().copied().fold(f32::NEG_INFINITY, f32::max)
        }));
        node.add_modifier(Modifier::Flat(3.0));
        node.add_modifier(Modifier::Flat(7.0));
        node.add_modifier(Modifier::Flat(1.0));
        assert_eq!(node.evaluate(&ctx), 7.0);
    }

    // --- Tagged modifier tests ---

    #[test]
    fn tagged_evaluate_filters_by_query() {
        let ctx = AttributeContext::new();
        let fire = TagMask::bit(0);
        let physical = TagMask::bit(1);
        let melee = TagMask::bit(2);

        let mut node = AttributeNode::sum();
        node.add_tagged_modifier(Modifier::Flat(25.0), physical | melee);
        node.add_tagged_modifier(Modifier::Flat(10.0), fire | melee);
        node.add_modifier(Modifier::Flat(5.0)); // global

        // Unfiltered: all modifiers
        assert_eq!(node.evaluate(&ctx), 40.0);

        // PHYSICAL|MELEE: physical+melee modifier (25) + global (5) = 30
        assert_eq!(node.evaluate_tagged(&ctx, physical | melee), 30.0);

        // FIRE|MELEE: fire+melee modifier (10) + global (5) = 15
        assert_eq!(node.evaluate_tagged(&ctx, fire | melee), 15.0);

        // MELEE only: global (5) only — neither tagged modifier is a subset
        assert_eq!(node.evaluate_tagged(&ctx, melee), 5.0);

        // FIRE|PHYSICAL|MELEE: all three match = 25 + 10 + 5 = 40
        assert_eq!(
            node.evaluate_tagged(&ctx, fire | physical | melee),
            40.0
        );
    }

    #[test]
    fn remove_tagged_modifier_matches_tag() {
        let ctx = AttributeContext::new();
        let fire = TagMask::bit(0);

        let mut node = AttributeNode::sum();
        node.add_tagged_modifier(Modifier::Flat(10.0), fire);
        node.add_modifier(Modifier::Flat(10.0)); // same value, NONE tag

        // Remove only the FIRE-tagged one
        assert!(node.remove_tagged_modifier(&Modifier::Flat(10.0), fire));
        assert_eq!(node.evaluate(&ctx), 10.0); // global remains
    }
}
