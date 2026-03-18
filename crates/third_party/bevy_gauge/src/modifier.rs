use crate::context::AttributeContext;
use crate::expr::Expr;
use crate::tags::TagMask;

/// A modifier that contributes a value to a attribute node.
///
/// Modifiers are either constant values or dynamic expressions
/// that reference other attributes.
#[derive(Clone, Debug)]
pub enum Modifier {
    /// A constant additive value.
    Flat(f32),
    /// A dynamic value computed from an expression referencing other attributes.
    Expr(Expr),
}

impl Modifier {
    /// Evaluate this modifier against a attribute context.
    pub fn evaluate(&self, context: &AttributeContext) -> f32 {
        match self {
            Modifier::Flat(val) => *val,
            Modifier::Expr(expr) => expr.evaluate(context),
        }
    }
}

impl PartialEq for Modifier {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Modifier::Flat(a), Modifier::Flat(b)) => (a - b).abs() < f32::EPSILON,
            (Modifier::Expr(a), Modifier::Expr(b)) => a == b,
            _ => false,
        }
    }
}

impl From<f32> for Modifier {
    fn from(val: f32) -> Self {
        Modifier::Flat(val)
    }
}

impl From<Expr> for Modifier {
    fn from(expr: Expr) -> Self {
        Modifier::Expr(expr)
    }
}

/// A modifier paired with a [`TagMask`] indicating which damage/attribute types
/// it applies to.
///
/// - A `TagMask::NONE` tag means the modifier is **global** — it participates
///   in every tag query (like PoE's "+20% increased damage").
/// - A non-empty tag (e.g. `FIRE | MELEE`) means the modifier only participates
///   in queries whose tag bits are a superset of the modifier's tag bits.
#[derive(Clone, Debug)]
pub struct TaggedModifier {
    pub modifier: Modifier,
    pub tag: TagMask,
}

impl TaggedModifier {
    /// Create a new tagged modifier.
    pub fn new(modifier: Modifier, tag: TagMask) -> Self {
        Self { modifier, tag }
    }

    /// Create a global (untagged) modifier that applies to every query.
    pub fn global(modifier: Modifier) -> Self {
        Self {
            modifier,
            tag: TagMask::NONE,
        }
    }
}

impl PartialEq for TaggedModifier {
    fn eq(&self, other: &Self) -> bool {
        self.modifier == other.modifier && self.tag == other.tag
    }
}
