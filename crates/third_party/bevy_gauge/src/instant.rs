//! One-shot attribute mutations — Set, Add, or Subtract a value once without
//! leaving a persistent modifier on the attribute node.
//!
//! [`InstantModifierSet`] is a portable collection of [`InstantEntry`] ops
//! that can be attached as a component (e.g., on ability effect entities) and
//! applied when triggered.
//!
//! # Role-based evaluation
//!
//! Expression values can reference attributes on **role entities** via the `@role`
//! syntax (e.g., `"Strength@attacker * 0.5"`). Roles are temporary source
//! aliases registered on the target entity for the duration of evaluation.
//!
//! # Example
//!
//! ```ignore
//! let instant = instant! {
//!     "Scorch" += 1.0,
//!     "Doom" += "-Doom@target",
//!     "ProjectileLife" -= 1.0,
//! };
//! attributes.apply_instant(&instant, &roles, defender);
//! ```

use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;

use crate::attribute_id::{global_rodeo, AttributeId};
use crate::attributes::Attributes;
use crate::attributes_mut::AttributesMut;
use crate::context::AttributeContext;
use crate::expr::{CompileError, Expr};
use crate::modifier_set::ModifierValue;
use crate::tags::TagMask;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// The operation to perform on a attribute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstantOp {
    /// Overwrite the attribute's value.
    Set,
    /// Add to the attribute's current value.
    Add,
    /// Subtract from the attribute's current value.
    Sub,
}

/// A single entry in an [`InstantModifierSet`].
#[derive(Clone, Debug)]
pub struct InstantEntry {
    /// The attribute path (e.g., `"Damage.base"`, `"Scorch"`).
    pub attribute: String,
    /// Which operation to perform.
    pub op: InstantOp,
    /// The value — either a literal f32 or an expression source string that
    /// is compiled at apply time.
    pub value: ModifierValue,
}

/// A portable collection of one-shot attribute operations.
///
/// Unlike [`ModifierSet`](crate::modifier_set::ModifierSet), which adds
/// persistent modifiers to attribute nodes, `InstantModifierSet` applies its
/// operations once and does not leave any modifiers behind.
///
/// Build one with the [`instant!`](crate::instant!) macro or the builder
/// methods, then apply it via [`apply_instant`].
#[derive(Component, Clone, Debug, Default)]
pub struct InstantModifierSet {
    pub(crate) entries: Vec<InstantEntry>,
}

impl InstantModifierSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a **Set** operation (overwrites the attribute value).
    pub fn push_set(&mut self, attribute: &str, value: impl Into<ModifierValue>) {
        self.entries.push(InstantEntry {
            attribute: attribute.to_string(),
            op: InstantOp::Set,
            value: value.into(),
        });
    }

    /// Push an **Add** operation (adds to the current attribute value).
    pub fn push_add(&mut self, attribute: &str, value: impl Into<ModifierValue>) {
        self.entries.push(InstantEntry {
            attribute: attribute.to_string(),
            op: InstantOp::Add,
            value: value.into(),
        });
    }

    /// Push a **Sub** operation (subtracts from the current attribute value).
    pub fn push_sub(&mut self, attribute: &str, value: impl Into<ModifierValue>) {
        self.entries.push(InstantEntry {
            attribute: attribute.to_string(),
            op: InstantOp::Sub,
            value: value.into(),
        });
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Evaluated entries
// ---------------------------------------------------------------------------

/// An [`InstantEntry`] after expression evaluation — holds a concrete f32.
#[derive(Clone, Debug)]
pub struct EvaluatedInstantEntry {
    pub attribute: String,
    pub op: InstantOp,
    pub value: f32,
    pub tag: Option<TagMask>,
}

// ---------------------------------------------------------------------------
// Role map
// ---------------------------------------------------------------------------

/// A slice of `(role_name, entity)` pairs used during role-based evaluation.
///
/// Example: `&[("attacker", attacker_entity), ("defender", defender_entity)]`
pub type RoleMap<'a> = &'a [(&'a str, Entity)];

// ---------------------------------------------------------------------------
// AttributeQueries — read-only expression evaluation trait
// ---------------------------------------------------------------------------

/// Read-only access to [`Attributes`] for expression evaluation.
///
/// Implemented for `Query<&Attributes, F>`, `Query<&mut Attributes, F>`, and
/// [`AttributesMut`]. This lets you call `evaluate_expr_with_roles` as a
/// method on any of these types without needing a separate query that would
/// cause Bevy system-param conflicts.
pub trait AttributeQueries {
    /// Get read-only access to an entity's [`Attributes`].
    fn get_attributes(&self, entity: Entity) -> Option<&Attributes>;

    /// Evaluate a compiled expression with role-entity source aliases.
    ///
    /// Builds a **temporary** evaluation context by cloning the target entity's
    /// cached values and populating any `@role` source references from the
    /// corresponding role entities. No mutation occurs.
    fn evaluate_expr_with_roles(
        &self,
        expr: &Expr,
        target_entity: Entity,
        roles: &[(&str, Entity)],
    ) -> f32 {
        self.evaluate_expr_with_roles_ctx(expr, target_entity, roles, None)
    }

    /// Like [`evaluate_expr_with_roles`](Self::evaluate_expr_with_roles) but
    /// injects additional ad-hoc `(name, value)` pairs into the evaluation
    /// context (e.g. `("initialHit", 42.0)`).
    fn evaluate_expr_with_roles_ctx(
        &self,
        expr: &Expr,
        target_entity: Entity,
        roles: &[(&str, Entity)],
        extra: Option<&[(&str, f32)]>,
    ) -> f32 {
        let rodeo = global_rodeo();

        let role_map: Vec<(AttributeId, Entity)> = roles
            .iter()
            .filter_map(|&(name, entity)| {
                rodeo.get(name).map(|spur| (AttributeId(spur), entity))
            })
            .collect();

        let mut ctx: AttributeContext = self
            .get_attributes(target_entity)
            .map(|a| a.context.clone())
            .unwrap_or_default();

        for (alias_id, attribute_id, cache_key, tag_mask) in expr.source_cache_keys() {
            let source_entity = role_map
                .iter()
                .find(|(id, _)| *id == alias_id)
                .copied()
                .map(|(_, e)| e);
            let value = source_entity
                .and_then(|e| self.get_attributes(e))
                .map(|attrs| match tag_mask {
                    Some(mask) => attrs.get_tagged(attribute_id, mask),
                    None => attrs.get(attribute_id),
                })
                .unwrap_or(0.0);
            ctx.set(cache_key, value);
        }

        if let Some(extras) = extra {
            for &(name, val) in extras {
                let spur = rodeo.get_or_intern(name);
                ctx.set(AttributeId(spur), val);
            }
        }

        expr.evaluate(&ctx)
    }
}

impl<'w, 's, F: QueryFilter> AttributeQueries for Query<'w, 's, &Attributes, F> {
    fn get_attributes(&self, entity: Entity) -> Option<&Attributes> {
        self.get(entity).ok()
    }
}

impl<'w, 's, F: QueryFilter> AttributeQueries for Query<'w, 's, &mut Attributes, F> {
    fn get_attributes(&self, entity: Entity) -> Option<&Attributes> {
        self.get(entity).ok()
    }
}

impl<'w, 's, F: QueryFilter> AttributeQueries for AttributesMut<'w, 's, F> {
    fn get_attributes(&self, entity: Entity) -> Option<&Attributes> {
        self.get_attributes(entity)
    }
}

// ---------------------------------------------------------------------------
// InstantExt — instant evaluate/apply as methods on AttributesMut
// ---------------------------------------------------------------------------

/// Extension trait that provides instant evaluate/apply methods on
/// [`AttributesMut`].
///
/// Turns free-function calls like `apply_instant(&instant, roles, target, &mut attributes)`
/// into method calls: `attributes.apply_instant(&instant, roles, target)`.
pub trait InstantExt {
    /// Evaluate all entries in an [`InstantModifierSet`] into concrete f32 values.
    ///
    /// Roles are registered as temporary source aliases on `target_entity` so
    /// that expressions like `"Strength@attacker"` resolve correctly. Aliases
    /// are cleaned up after evaluation.
    fn evaluate_instant(
        &mut self,
        instant: &InstantModifierSet,
        roles: RoleMap,
        target_entity: Entity,
    ) -> Vec<EvaluatedInstantEntry>;

    /// Apply previously evaluated instant operations to a specific entity.
    fn apply_evaluated_instant(
        &mut self,
        evaluated: &[EvaluatedInstantEntry],
        target_entity: Entity,
    );

    /// Evaluate and immediately apply an [`InstantModifierSet`] to a target entity.
    fn apply_instant(
        &mut self,
        instant: &InstantModifierSet,
        roles: RoleMap,
        target_entity: Entity,
    );

    /// Evaluate an [`InstantModifierSet`] with dynamic tag parameters.
    ///
    /// Tag placeholders in expression strings (e.g., `{%element%}`) are
    /// substituted with their resolved tag names before compilation.
    fn evaluate_instant_with_tags(
        &mut self,
        instant: &InstantModifierSet,
        roles: RoleMap,
        tag_params: &[(&str, TagMask)],
        target_entity: Entity,
    ) -> Result<Vec<EvaluatedInstantEntry>, CompileError>;

    /// Evaluate and immediately apply an [`InstantModifierSet`] with dynamic tag
    /// parameters.
    fn apply_instant_with_tags(
        &mut self,
        instant: &InstantModifierSet,
        roles: RoleMap,
        tag_params: &[(&str, TagMask)],
        target_entity: Entity,
    ) -> Result<(), CompileError>;
}

impl<'w, 's, F: QueryFilter> InstantExt for AttributesMut<'w, 's, F> {
    fn evaluate_instant(
        &mut self,
        instant: &InstantModifierSet,
        roles: RoleMap,
        target_entity: Entity,
    ) -> Vec<EvaluatedInstantEntry> {
        for &(role_name, role_entity) in roles {
            self.register_source(target_entity, role_name, role_entity);
        }

        let mut out = Vec::with_capacity(instant.entries.len());

        for entry in &instant.entries {
            let (base_attr, tag) = parse_attribute_tag(&entry.attribute, self.tag_resolver())
                .unwrap_or_else(|_| (entry.attribute.clone(), None));

            let value = match &entry.value {
                ModifierValue::Literal(v) => *v,
                ModifierValue::ExprSource(src) => {
                    let expr = crate::expr::Expr::compile(
                        src,
                        Some(self.tag_resolver()),
                    );
                    match expr {
                        Ok(compiled) => {
                            self.cache_expr_source_values(target_entity, &compiled);
                            match AttributeQueries::get_attributes(self, target_entity) {
                                Some(attrs) => compiled.evaluate(&attrs.context),
                                None => 0.0,
                            }
                        }
                        Err(_) => 0.0,
                    }
                }
            };

            out.push(EvaluatedInstantEntry {
                attribute: base_attr,
                op: entry.op.clone(),
                value,
                tag,
            });
        }

        for &(role_name, _) in roles {
            self.unregister_source(target_entity, role_name);
        }

        out
    }

    fn apply_evaluated_instant(
        &mut self,
        evaluated: &[EvaluatedInstantEntry],
        target_entity: Entity,
    ) {
        for entry in evaluated {
            match (&entry.op, entry.tag) {
                (InstantOp::Set, Some(tag)) => {
                    self.set_base_tagged(target_entity, &entry.attribute, entry.value, tag);
                }
                (InstantOp::Set, None) => {
                    self.set_base(target_entity, &entry.attribute, entry.value);
                }
                (InstantOp::Add, Some(tag)) => {
                    let current = self.evaluate_tagged(target_entity, &entry.attribute, tag);
                    self.set_base_tagged(target_entity, &entry.attribute, current + entry.value, tag);
                }
                (InstantOp::Add, None) => {
                    let current = self.evaluate(target_entity, &entry.attribute);
                    self.set_base(target_entity, &entry.attribute, current + entry.value);
                }
                (InstantOp::Sub, Some(tag)) => {
                    let current = self.evaluate_tagged(target_entity, &entry.attribute, tag);
                    self.set_base_tagged(target_entity, &entry.attribute, current - entry.value, tag);
                }
                (InstantOp::Sub, None) => {
                    let current = self.evaluate(target_entity, &entry.attribute);
                    self.set_base(target_entity, &entry.attribute, current - entry.value);
                }
            }
        }
    }

    fn apply_instant(
        &mut self,
        instant: &InstantModifierSet,
        roles: RoleMap,
        target_entity: Entity,
    ) {
        let evaluated = self.evaluate_instant(instant, roles, target_entity);
        self.apply_evaluated_instant(&evaluated, target_entity);
    }

    fn evaluate_instant_with_tags(
        &mut self,
        instant: &InstantModifierSet,
        roles: RoleMap,
        tag_params: &[(&str, TagMask)],
        target_entity: Entity,
    ) -> Result<Vec<EvaluatedInstantEntry>, CompileError> {
        let resolved = substitute_tag_params(instant, tag_params, self.tag_resolver())?;
        Ok(self.evaluate_instant(&resolved, roles, target_entity))
    }

    fn apply_instant_with_tags(
        &mut self,
        instant: &InstantModifierSet,
        roles: RoleMap,
        tag_params: &[(&str, TagMask)],
        target_entity: Entity,
    ) -> Result<(), CompileError> {
        let evaluated = self.evaluate_instant_with_tags(instant, roles, tag_params, target_entity)?;
        self.apply_evaluated_instant(&evaluated, target_entity);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Attribute name tag parsing (internal helper)
// ---------------------------------------------------------------------------

/// Parse tag syntax from an instant attribute name.
///
/// `"Status{POISON}"` → `("Status", Some(poison_mask))`
/// `"Life.current"` → `("Life.current", None)`
fn parse_attribute_tag(
    attr: &str,
    resolver: &crate::tags::TagResolver,
) -> Result<(String, Option<TagMask>), CompileError> {
    let Some(brace_start) = attr.find('{') else {
        return Ok((attr.to_string(), None));
    };
    let Some(brace_end) = attr[brace_start..].find('}') else {
        return Ok((attr.to_string(), None));
    };
    let brace_end = brace_start + brace_end;

    let base = attr[..brace_start].to_string();
    let tag_body = &attr[brace_start + 1..brace_end];

    let mut mask = TagMask::NONE;
    for part in tag_body.split('|') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        match resolver.resolve(trimmed) {
            Some(m) => mask = mask | m,
            None => {
                if let Some(alts) = resolver.ambiguous_alternatives(trimmed) {
                    return Err(CompileError::AmbiguousTag(
                        trimmed.to_uppercase(),
                        alts,
                    ));
                }
                return Err(CompileError::UnknownTag(trimmed.to_string()));
            }
        }
    }

    Ok((base, Some(mask)))
}

// ---------------------------------------------------------------------------
// Tag-parameterized instants (internal helper)
// ---------------------------------------------------------------------------

/// Substitute `{%name%}` placeholders in expression source strings and
/// attribute names with the resolved tag syntax (e.g., `{FIRE}` or `{FIRE|SPELL}`).
fn substitute_tag_params(
    instant: &InstantModifierSet,
    tag_params: &[(&str, TagMask)],
    resolver: &crate::tags::TagResolver,
) -> Result<InstantModifierSet, CompileError> {
    let mut replacements: Vec<(String, String)> = Vec::with_capacity(tag_params.len());
    for &(name, mask) in tag_params {
        let suffix = resolver
            .tag_suffix(mask)
            .ok_or(CompileError::UnresolvableTagMask(mask))?;
        let placeholder = format!("{{%{name}%}}");
        replacements.push((placeholder, suffix));
    }

    let mut out = InstantModifierSet::new();
    for entry in &instant.entries {
        let mut attr = entry.attribute.clone();
        for (placeholder, replacement) in &replacements {
            attr = attr.replace(placeholder, replacement);
        }

        let value = match &entry.value {
            ModifierValue::ExprSource(src) => {
                let mut s = src.clone();
                for (placeholder, replacement) in &replacements {
                    s = s.replace(placeholder, replacement);
                }
                ModifierValue::ExprSource(s)
            }
            other => other.clone(),
        };
        out.entries.push(InstantEntry {
            attribute: attr,
            op: entry.op.clone(),
            value,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// instant! macro
// ---------------------------------------------------------------------------

/// Create an [`InstantModifierSet`] from a declarative list of operations.
///
/// # Syntax
///
/// ```ignore
/// instant! {
///     "AttributeName" = value,             // Set
///     "AttributeName" += value,            // Add
///     "AttributeName" -= value,            // Sub
///     "Status{POISON}" += "Buildup@weapon", // Tagged attribute
/// }
/// ```
///
/// - **`value`** can be an `f32` literal or a string expression
///   (e.g., `"-Doom@target"`).
/// - Attribute names can include `{TAG}` syntax to target tagged attribute
///   slots (e.g., `"Status{POISON}"`, `"Status{%status%}"`).
///
/// # Example
///
/// ```ignore
/// let effects = instant! {
///     "Scorch" += 1.0,
///     "Doom" += "-Doom@target",
///     "ProjectileLife" -= 1.0,
///     "Health" = "Strength@attacker * 0.5",
/// };
/// ```
#[macro_export]
macro_rules! instant {
    { $( $attribute:literal $op:tt $value:expr ),* $(,)? } => {{
        let mut _set = $crate::instant::InstantModifierSet::new();
        $(
            $crate::instant!(@entry _set, $attribute, $op, $value);
        )*
        _set
    }};

    (@entry $set:ident, $attribute:literal, +=, $value:expr) => {
        $set.push_add($attribute, $value);
    };
    (@entry $set:ident, $attribute:literal, -=, $value:expr) => {
        $set.push_sub($attribute, $value);
    };
    (@entry $set:ident, $attribute:literal, =, $value:expr) => {
        $set.push_set($attribute, $value);
    };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::TagResolver;

    #[test]
    fn substitute_single_tag_param() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));

        let mut instant = InstantModifierSet::new();
        instant.push_sub(
            "Life.current",
            ModifierValue::ExprSource("Damage{%element%}@weapon".into()),
        );

        let result = substitute_tag_params(
            &instant,
            &[("element", TagMask::bit(0))],
            &resolver,
        )
        .unwrap();

        match &result.entries[0].value {
            ModifierValue::ExprSource(s) => {
                assert_eq!(s, "Damage{FIRE}@weapon");
            }
            other => panic!("expected ExprSource, got {:?}", other),
        }
    }

    #[test]
    fn substitute_multi_tag_param() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        resolver.register("SPELL", TagMask::bit(3));

        let mut instant = InstantModifierSet::new();
        instant.push_sub(
            "Life.current",
            ModifierValue::ExprSource("Damage{%element%}@weapon".into()),
        );

        let mask = TagMask::bit(0) | TagMask::bit(3);
        let result = substitute_tag_params(&instant, &[("element", mask)], &resolver).unwrap();

        match &result.entries[0].value {
            ModifierValue::ExprSource(s) => {
                assert!(s.contains("FIRE"));
                assert!(s.contains("SPELL"));
                assert!(s.starts_with("Damage{"));
                assert!(s.contains("}@weapon"));
            }
            other => panic!("expected ExprSource, got {:?}", other),
        }
    }

    #[test]
    fn substitute_preserves_literals() {
        let resolver = TagResolver::new();
        let mut instant = InstantModifierSet::new();
        instant.push_add("Scorch", 5.0f32);

        let result = substitute_tag_params(&instant, &[], &resolver).unwrap();
        match &result.entries[0].value {
            ModifierValue::Literal(v) => assert_eq!(*v, 5.0),
            other => panic!("expected Literal, got {:?}", other),
        }
    }

    #[test]
    fn substitute_multiple_placeholders() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        resolver.register("PHYSICAL", TagMask::bit(1));

        let mut instant = InstantModifierSet::new();
        instant.push_sub(
            "Life.current",
            ModifierValue::ExprSource(
                "Damage{%element%}@weapon * (1 - Resistance{%element%})".into(),
            ),
        );

        let result = substitute_tag_params(
            &instant,
            &[("element", TagMask::bit(0))],
            &resolver,
        )
        .unwrap();

        match &result.entries[0].value {
            ModifierValue::ExprSource(s) => {
                assert_eq!(s, "Damage{FIRE}@weapon * (1 - Resistance{FIRE})");
            }
            other => panic!("expected ExprSource, got {:?}", other),
        }
    }

    #[test]
    fn substitute_unresolvable_mask_errors() {
        let resolver = TagResolver::new();

        let mut instant = InstantModifierSet::new();
        instant.push_sub(
            "Life.current",
            ModifierValue::ExprSource("Damage{%element%}@weapon".into()),
        );

        let result = substitute_tag_params(
            &instant,
            &[("element", TagMask::bit(5))],
            &resolver,
        );
        assert!(matches!(result, Err(CompileError::UnresolvableTagMask(_))));
    }

    // --- parse_attribute_tag tests ---

    #[test]
    fn parse_untagged_attribute() {
        let resolver = TagResolver::new();
        let (name, tag) = parse_attribute_tag("Life.current", &resolver).unwrap();
        assert_eq!(name, "Life.current");
        assert!(tag.is_none());
    }

    #[test]
    fn parse_single_tag_attribute() {
        let mut resolver = TagResolver::new();
        resolver.register("POISON", TagMask::bit(0));
        let (name, tag) = parse_attribute_tag("Status{POISON}", &resolver).unwrap();
        assert_eq!(name, "Status");
        assert_eq!(tag, Some(TagMask::bit(0)));
    }

    #[test]
    fn parse_multi_tag_attribute() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        resolver.register("SWORD", TagMask::bit(1));
        let (name, tag) = parse_attribute_tag("Damage{FIRE|SWORD}", &resolver).unwrap();
        assert_eq!(name, "Damage");
        assert_eq!(tag, Some(TagMask::bit(0) | TagMask::bit(1)));
    }

    #[test]
    fn parse_unknown_tag_errors() {
        let resolver = TagResolver::new();
        let result = parse_attribute_tag("Status{NOPE}", &resolver);
        assert!(matches!(result, Err(CompileError::UnknownTag(_))));
    }

    // --- substitute_tag_params in attribute names ---

    #[test]
    fn substitute_tag_in_attribute_name() {
        let mut resolver = TagResolver::new();
        resolver.register("POISON", TagMask::bit(0));

        let mut instant = InstantModifierSet::new();
        instant.push_add("Status{%status%}", 10.0f32);

        let result = substitute_tag_params(
            &instant,
            &[("status", TagMask::bit(0))],
            &resolver,
        )
        .unwrap();

        assert_eq!(result.entries[0].attribute, "Status{POISON}");
    }
}
