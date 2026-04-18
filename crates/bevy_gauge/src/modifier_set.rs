use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;

use crate::attributes_mut::AttributesMut;
use crate::tags::TagMask;

/// How a modifier value is stored before application.
///
/// - `Literal` values become `Modifier::Flat` when applied.
/// - `ExprSource` values are compiled to `Modifier::Expr` when applied (at
///   which point the `Interner` and `TagResolver` are available).
#[derive(Clone, Debug)]
pub enum ModifierValue {
    /// A constant f32 value.
    Literal(f32),
    /// An expression source string to be compiled at apply time.
    ExprSource(String),
}

impl From<f32> for ModifierValue {
    fn from(val: f32) -> Self {
        ModifierValue::Literal(val)
    }
}

impl From<&str> for ModifierValue {
    fn from(s: &str) -> Self {
        ModifierValue::ExprSource(s.to_string())
    }
}

impl From<String> for ModifierValue {
    fn from(s: String) -> Self {
        ModifierValue::ExprSource(s)
    }
}

/// A single entry in a [`ModifierSet`].
#[derive(Clone, Debug)]
pub struct ModifierEntry {
    /// The attribute path (e.g., `"Damage.Added"`).
    pub attribute: String,
    /// The modifier value — either a literal or an expression source string.
    pub value: ModifierValue,
    /// Tag mask for the modifier. `TagMask::NONE` means global.
    pub tag: TagMask,
}

/// A portable collection of modifiers that can be applied to an entity.
///
/// Build one manually or via the [`attributes!`] / [`mod_set!`] macros.
/// Apply it to an entity by spawning it as [`AttributeInitializer`] or by
/// calling [`apply`](Self::apply) directly with an [`AttributesMut`].
///
/// # Example
///
/// ```ignore
/// let mut set = ModifierSet::new();
/// set.add("Strength", 50.0);
/// set.add_tagged("Damage.Added", 25.0, FIRE | MELEE);
/// set.add_expr("Health", "Strength * 2.0");
/// set.apply(entity, &mut attributes);
/// ```
#[derive(Clone, Debug, Default)]
pub struct ModifierSet {
    pub(crate) entries: Vec<ModifierEntry>,
}

impl ModifierSet {
    /// Create a new empty modifier set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an untagged modifier (literal f32 or expression string).
    pub fn add(&mut self, attribute: &str, value: impl Into<ModifierValue>) {
        self.entries.push(ModifierEntry {
            attribute: attribute.to_string(),
            value: value.into(),
            tag: TagMask::NONE,
        });
    }

    /// Add a tagged modifier (literal f32 or expression string).
    pub fn add_tagged(&mut self, attribute: &str, value: impl Into<ModifierValue>, tag: TagMask) {
        self.entries.push(ModifierEntry {
            attribute: attribute.to_string(),
            value: value.into(),
            tag,
        });
    }

    /// Add an untagged expression modifier from a source string.
    pub fn add_expr(&mut self, attribute: &str, expr_source: &str) {
        self.add(attribute, ModifierValue::ExprSource(expr_source.to_string()));
    }

    /// Add a tagged expression modifier from a source string.
    pub fn add_expr_tagged(&mut self, attribute: &str, expr_source: &str, tag: TagMask) {
        self.add_tagged(
            attribute,
            ModifierValue::ExprSource(expr_source.to_string()),
            tag,
        );
    }

    /// Apply all modifiers in this set to an entity via `AttributesMut`.
    ///
    /// Literal values are added as flat modifiers. Expression strings are
    /// compiled and added as expression modifiers (compilation errors are
    /// silently ignored — use `try_apply` for error handling).
    pub fn apply<F: QueryFilter>(&self, entity: Entity, attributes: &mut AttributesMut<'_, '_, F>) {
        for entry in &self.entries {
            match &entry.value {
                ModifierValue::Literal(val) => {
                    attributes.add_modifier_tagged(entity, &entry.attribute, *val, entry.tag);
                }
                ModifierValue::ExprSource(src) => {
                    if entry.tag.is_empty() {
                        let _ = attributes.add_expr_modifier(entity, &entry.attribute, src);
                    } else {
                        let _ = attributes.add_expr_modifier_tagged(entity, &entry.attribute, src, entry.tag);
                    }
                }
            }
        }
    }

    /// Apply all modifiers, returning errors for any expression compilation failures.
    pub fn try_apply<F: QueryFilter>(
        &self,
        entity: Entity,
        attributes: &mut AttributesMut<'_, '_, F>,
    ) -> Result<(), crate::expr::CompileError> {
        for entry in &self.entries {
            match &entry.value {
                ModifierValue::Literal(val) => {
                    attributes.add_modifier_tagged(entity, &entry.attribute, *val, entry.tag);
                }
                ModifierValue::ExprSource(src) => {
                    if entry.tag.is_empty() {
                        attributes.add_expr_modifier(entity, &entry.attribute, src)?;
                    } else {
                        attributes.add_expr_modifier_tagged(entity, &entry.attribute, src, entry.tag)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Remove all modifiers in this set from an entity via `AttributesMut`.
    ///
    /// This is the inverse of [`apply`](Self::apply). Literal values are removed
    /// as flat modifiers. Expression strings are recompiled and removed as
    /// expression modifiers (compilation errors are silently ignored).
    pub fn remove<F: QueryFilter>(&self, entity: Entity, attributes: &mut AttributesMut<'_, '_, F>) {
        for entry in &self.entries {
            match &entry.value {
                ModifierValue::Literal(val) => {
                    let modifier = crate::modifier::Modifier::Flat(*val);
                    attributes.remove_modifier_tagged(entity, &entry.attribute, &modifier, entry.tag);
                }
                ModifierValue::ExprSource(src) => {
                    if let Ok(expr) = crate::expr::Expr::compile(
                        src,
                        Some(attributes.tag_resolver()),
                    ) {
                        let modifier = crate::modifier::Modifier::Expr(expr);
                        attributes.remove_modifier_tagged(entity, &entry.attribute, &modifier, entry.tag);
                    }
                }
            }
        }
    }

    /// Remove all modifiers, returning errors for any expression compilation failures.
    pub fn try_remove<F: QueryFilter>(
        &self,
        entity: Entity,
        attributes: &mut AttributesMut<'_, '_, F>,
    ) -> Result<(), crate::expr::CompileError> {
        for entry in &self.entries {
            match &entry.value {
                ModifierValue::Literal(val) => {
                    let modifier = crate::modifier::Modifier::Flat(*val);
                    attributes.remove_modifier_tagged(entity, &entry.attribute, &modifier, entry.tag);
                }
                ModifierValue::ExprSource(src) => {
                    let expr = crate::expr::Expr::compile(
                        src,
                        Some(attributes.tag_resolver()),
                    )?;
                    let modifier = crate::modifier::Modifier::Expr(expr);
                    attributes.remove_modifier_tagged(entity, &entry.attribute, &modifier, entry.tag);
                }
            }
        }
        Ok(())
    }

    /// Append all entries from another modifier set into this one.
    pub fn combine(&mut self, other: &ModifierSet) {
        self.entries.extend(other.entries.iter().cloned());
    }

    /// Number of entries in this set.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether this set is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A component that carries a [`ModifierSet`] to be applied on spawn.
///
/// When this component is added to an entity that also has [`Attributes`],
/// the modifiers are automatically applied via an observer, and the
/// `AttributeInitializer` component is removed.
///
/// # Example
///
/// ```ignore
/// commands.spawn((
///     Attributes::new(),
///     AttributeInitializer::new(my_modifier_set),
/// ));
/// ```
///
/// Or with the [`attributes!`] macro:
///
/// ```ignore
/// commands.spawn((
///     Attributes::new(),
///     attributes! {
///         "Strength" => 50.0,
///         "Health" => "Strength * 2.0",
///     },
/// ));
/// ```
#[derive(Component, Clone, Debug)]
#[require(crate::prelude::Attributes)]
pub struct AttributeInitializer(pub ModifierSet);

impl AttributeInitializer {
    /// Create a new `AttributeInitializer` from a modifier set.
    pub fn new(set: ModifierSet) -> Self {
        Self(set)
    }
}

/// Observer that applies `AttributeInitializer` when the component is added.
pub(crate) fn apply_initial_attributes(
    trigger: On<Add, AttributeInitializer>,
    initial_query: Query<&AttributeInitializer>,
    mut attributes: AttributesMut,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    if let Ok(initial) = initial_query.get(entity) {
        initial.0.apply(entity, &mut attributes);
    }
    // Remove the component now that it's been applied
    commands.entity(entity).remove::<AttributeInitializer>();
}
