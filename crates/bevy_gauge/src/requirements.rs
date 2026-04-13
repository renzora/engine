//! Boolean attribute requirements — expressions evaluated against an entity's
//! [`Attributes`] that gate state-machine transitions, equipment prerequisites,
//! ability conditions, etc.
//!
//! # Example
//!
//! ```ignore
//! // As a component on a state-machine edge:
//! commands.spawn((
//!     requires! { "ProjectileLife <= 0" },
//!     // ...
//! ));
//!
//! // Checking requirements in a system:
//! if attribute_requirements.met(&attrs) {
//!     // all requirements satisfied
//! }
//! ```

use bevy::prelude::*;

use crate::attributes::Attributes;
use crate::expr::Expr;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single boolean requirement over attributes.
///
/// The expression is compiled lazily on first [`met`](Self::met) call using
/// the global [`Interner`](crate::attribute_id::Interner).
#[derive(Clone, Debug)]
pub struct AttributeRequirement {
    source: String,
    compiled: Option<Expr>,
}

impl AttributeRequirement {
    /// Create a new requirement from an expression string.
    ///
    /// The expression is stored but not compiled until first evaluation.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            compiled: None,
        }
    }

    /// Check whether this requirement is satisfied against the given attributes.
    ///
    /// The expression is compiled on first call (using the global interner)
    /// and cached for subsequent evaluations. Returns `true` if the expression
    /// evaluates to a non-zero value (truthy).
    pub fn met(&mut self, attrs: &Attributes) -> bool {
        let expr = match &self.compiled {
            Some(expr) => expr,
            None => {
                match Expr::compile(&self.source, None) {
                    Ok(expr) => {
                        self.compiled = Some(expr);
                        self.compiled.as_ref().unwrap()
                    }
                    Err(err) => {
                        warn!("AttributeRequirement compile error for '{}': {}", self.source, err);
                        return false;
                    }
                }
            }
        };
        expr.evaluate(&attrs.context) != 0.0
    }

    /// Get the source expression string.
    pub fn source(&self) -> &str {
        &self.source
    }
}

impl<S: Into<String>> From<S> for AttributeRequirement {
    fn from(value: S) -> Self {
        Self::new(value)
    }
}

// ---------------------------------------------------------------------------
// AttributeRequirements component
// ---------------------------------------------------------------------------

/// A component holding one or more boolean attribute requirements.
///
/// All requirements must be satisfied for [`met`](Self::met) to return `true`.
#[derive(Component, Debug, Default, Clone)]
pub struct AttributeRequirements(pub Vec<AttributeRequirement>);

impl AttributeRequirements {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a requirement from an expression string.
    pub fn add(&mut self, expr: impl Into<String>) {
        self.0.push(AttributeRequirement::new(expr));
    }

    /// Merge requirements from another set into this one.
    pub fn combine(&mut self, other: &AttributeRequirements) {
        self.0.extend(other.0.iter().cloned());
    }

    /// Check whether **all** requirements are satisfied.
    ///
    /// Returns `true` if the list is empty (vacuous truth).
    pub fn met(&mut self, attrs: &Attributes) -> bool {
        self.0.iter_mut().all(|req| req.met(attrs))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<I, S> From<I> for AttributeRequirements
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    fn from(value: I) -> Self {
        Self(value.into_iter().map(AttributeRequirement::new).collect())
    }
}

// ---------------------------------------------------------------------------
// requires! macro
// ---------------------------------------------------------------------------

/// Create a [`AttributeRequirements`] component from one or more expression strings.
///
/// # Example
///
/// ```ignore
/// requires! { "ProjectileLife <= 0" }
/// requires! { "Strength >= 10", "Level >= 5" }
/// ```
#[macro_export]
macro_rules! requires {
    { $( $expr:literal ),* $(,)? } => {{
        let mut _reqs = $crate::requirements::AttributeRequirements::new();
        $(
            _reqs.add($expr);
        )*
        _reqs
    }};
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute_id::Interner;
    use crate::modifier::Modifier;
    use crate::node::ReduceFn;

    fn test_interner() -> Interner {
        // First call creates and sets the global; subsequent calls are no-ops.
        let i = Interner::new();
        i.set_global();
        Interner::global()
    }

    fn make_attrs(interner: &Interner, attributes: &[(&str, f32)]) -> Attributes {
        let mut attrs = Attributes::new();
        for &(name, value) in attributes {
            let id = interner.get_or_intern(name);
            let node = attrs.ensure_node(id, ReduceFn::Sum);
            node.add_modifier(Modifier::Flat(value));
            attrs.evaluate_and_cache(id);
        }
        attrs
    }

    #[test]
    fn single_requirement_met() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("Strength", 25.0)]);
        let mut req = AttributeRequirement::new("Strength >= 10");
        assert!(req.met(&attrs));
    }

    #[test]
    fn single_requirement_not_met() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("Strength", 5.0)]);
        let mut req = AttributeRequirement::new("Strength >= 10");
        assert!(!req.met(&attrs));
    }

    #[test]
    fn requirements_all_met() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("Strength", 25.0), ("Level", 10.0)]);
        let mut reqs = AttributeRequirements::from(vec!["Strength >= 10", "Level >= 5"]);
        assert!(reqs.met(&attrs));
    }

    #[test]
    fn requirements_partial_met() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("Strength", 25.0), ("Level", 3.0)]);
        let mut reqs = AttributeRequirements::from(vec!["Strength >= 10", "Level >= 5"]);
        assert!(!reqs.met(&attrs));
    }

    #[test]
    fn empty_requirements_are_met() {
        let attrs = Attributes::new();
        let mut reqs = AttributeRequirements::new();
        assert!(reqs.met(&attrs));
    }

    #[test]
    fn le_zero_check() {
        let interner = test_interner();
        let attrs = make_attrs(&interner, &[("ProjectileLife", 0.0)]);
        let mut req = AttributeRequirement::new("ProjectileLife <= 0");
        assert!(req.met(&attrs));
    }

    #[test]
    fn combine_merges() {
        let a = AttributeRequirements::from(vec!["A >= 1"]);
        let b = AttributeRequirements::from(vec!["B >= 2", "C >= 3"]);
        let mut combined = a;
        combined.combine(&b);
        assert_eq!(combined.len(), 3);
    }
}
