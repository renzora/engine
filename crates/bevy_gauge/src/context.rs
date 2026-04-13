use std::collections::HashMap;

use crate::attribute_id::AttributeId;

/// Sparse evaluation context — maps AttributeIds to their current f32 values.
///
/// This is the data structure that expressions read from during evaluation.
/// Unset attributes default to 0.0.
#[derive(Clone, Debug, Default)]
pub struct AttributeContext {
    values: HashMap<AttributeId, f32>,
}

impl AttributeContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the current value of a attribute. Returns 0.0 if the attribute hasn't been set.
    pub fn get(&self, id: AttributeId) -> f32 {
        self.values.get(&id).copied().unwrap_or(0.0)
    }

    /// Set the value of a attribute.
    pub fn set(&mut self, id: AttributeId, value: f32) {
        self.values.insert(id, value);
    }

    /// Remove a attribute from the context.
    pub fn remove(&mut self, id: AttributeId) {
        self.values.remove(&id);
    }

    /// Check if a attribute has a value in this context.
    pub fn contains(&self, id: AttributeId) -> bool {
        self.values.contains_key(&id)
    }

    /// Iterate over all (AttributeId, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (AttributeId, f32)> + '_ {
        self.values.iter().map(|(&id, &val)| (id, val))
    }

    /// Number of attributes in the context.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the context is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute_id::Interner;

    #[test]
    fn default_value_is_zero() {
        let interner = Interner::new();
        let ctx = AttributeContext::new();
        let id = interner.get_or_intern("Missing");
        assert_eq!(ctx.get(id), 0.0);
    }

    #[test]
    fn set_and_get() {
        let interner = Interner::new();
        let mut ctx = AttributeContext::new();
        let id = interner.get_or_intern("Strength");
        ctx.set(id, 42.0);
        assert_eq!(ctx.get(id), 42.0);
    }

    #[test]
    fn overwrite() {
        let interner = Interner::new();
        let mut ctx = AttributeContext::new();
        let id = interner.get_or_intern("Health");
        ctx.set(id, 100.0);
        ctx.set(id, 75.0);
        assert_eq!(ctx.get(id), 75.0);
    }

    #[test]
    fn remove_attribute() {
        let interner = Interner::new();
        let mut ctx = AttributeContext::new();
        let id = interner.get_or_intern("Mana");
        ctx.set(id, 50.0);
        ctx.remove(id);
        assert_eq!(ctx.get(id), 0.0);
        assert!(!ctx.contains(id));
    }
}
