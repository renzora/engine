//! Derived component support — automatically sync Bevy components with attributes.
//!
//! # Overview
//!
//! [`AttributeDerived`] components are **read-from** — their fields are updated
//! from attribute values every frame (when changed).
//!
//! [`WriteBack`] components are **write-to** — their fields are written
//! back to the attribute system every frame (when changed).
//!
//! Register derived components via the [`AttributesAppExt`] extension trait:
//!
//! ```ignore
//! app.register_attribute_derived::<PlayerHealth>()
//!     .register_write_back::<PlayerInput>();
//! ```

use bevy::ecs::component::Mutable;
use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;

use crate::attributes::Attributes;
use crate::attributes_mut::AttributesMut;

// ---------------------------------------------------------------------------
// System sets
// ---------------------------------------------------------------------------

/// System set for systems that write [`WriteBack`] component values into attributes.
/// Runs in both `PreUpdate` and `PostUpdate`, before [`AttributeDerivedSet`].
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct WriteBackSet;

/// System set for systems that update [`AttributeDerived`] components from attributes.
/// Runs in both `PreUpdate` and `PostUpdate`, after [`WriteBackSet`].
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttributeDerivedSet;

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// A component whose fields are populated from attribute values.
///
/// Implement this trait (manually or via `attribute_component!`) to have a
/// component automatically updated when its source attributes change.
///
/// # Example
///
/// ```ignore
/// #[derive(Component, Default)]
/// struct PlayerHealth {
///     current: f32,
///     max: f32,
/// }
///
/// impl AttributeDerived for PlayerHealth {
///     fn should_update(&self, attrs: &Attributes) -> bool {
///         let max = attrs.value("Health.Max");
///         let current = attrs.value("Health.Current");
///         (self.max - max).abs() > f32::EPSILON
///             || (self.current - current).abs() > f32::EPSILON
///     }
///
///     fn update_from_attributes(&mut self, attrs: &Attributes) {
///         self.max = attrs.value("Health.Max");
///         self.current = attrs.value("Health.Current");
///     }
/// }
/// ```
pub trait AttributeDerived: Component<Mutability = Mutable> {
    /// Check whether this component's fields are out of date relative to attributes.
    fn should_update(&self, attrs: &Attributes) -> bool;

    /// Update this component's fields from attribute values.
    fn update_from_attributes(&mut self, attrs: &Attributes);
}

/// A component whose fields are written back into the attribute system.
///
/// Implement this for components that are authoritative over certain attribute
/// values — e.g., an input component that controls a attribute directly.
///
/// # Example
///
/// ```ignore
/// #[derive(Component)]
/// struct CombatInput {
///     attack_power_override: f32,
/// }
///
/// impl WriteBack for CombatInput {
///     fn should_write_back(&self, attrs: &Attributes) -> bool {
///         let current = attrs.value("AttackPower.Override");
///         (self.attack_power_override - current).abs() > f32::EPSILON
///     }
///
///     fn write_back<F: QueryFilter>(&self, entity: Entity, attributes: &mut AttributesMut<'_, '_, F>) {
///         attributes.set(entity, "AttackPower.Override", self.attack_power_override);
///     }
/// }
/// ```
pub trait WriteBack: Component {
    /// Check whether this component has values that differ from current attributes.
    fn should_write_back(&self, attrs: &Attributes) -> bool;

    /// Write this component's values into the attribute system.
    fn write_back<F: QueryFilter>(&self, entity: Entity, attributes: &mut AttributesMut<'_, '_, F>);
}

// ---------------------------------------------------------------------------
// Generic systems
// ---------------------------------------------------------------------------

/// Generic system that updates all entities with a `AttributeDerived` component.
///
/// Only runs for entities whose [`Attributes`] changed since last tick.
pub fn update_attribute_derived<T: AttributeDerived>(
    mut query: Query<(&mut T, &Attributes), Changed<Attributes>>,
) {
    for (mut derived, attrs) in &mut query {
        if derived.should_update(attrs) {
            derived.update_from_attributes(attrs);
        }
    }
}

/// Generic system that writes back all entities with a changed `WriteBack` component.
///
/// Only runs for entities whose `T` component changed since last tick.
/// The `should_write_back` guard prevents unnecessary attribute writes when the
/// component was mutably accessed but its values didn't actually change.
pub fn update_write_back<T: WriteBack>(
    q_wb: Query<(Entity, &T), Changed<T>>,
    mut attributes: AttributesMut,
) {
    for (entity, wb) in &q_wb {
        let should = {
            let Some(attrs) = attributes.get_attributes(entity) else {
                continue;
            };
            wb.should_write_back(attrs)
        };
        if should {
            wb.write_back(entity, &mut attributes);
        }
    }
}

// ---------------------------------------------------------------------------
// App extension trait
// ---------------------------------------------------------------------------

/// Extension trait for registering derived attribute components with the Bevy app.
pub trait AttributesAppExt {
    /// Register a [`AttributeDerived`] component.
    ///
    /// Adds sync systems to both [`PreUpdate`] and [`PostUpdate`] (in the
    /// [`AttributeDerivedSet`]). The `PreUpdate` pass ensures components are
    /// fresh before `Update` gameplay systems run; the `PostUpdate` pass
    /// catches attribute changes made during `Update`.
    fn register_attribute_derived<T: AttributeDerived>(&mut self) -> &mut Self;

    /// Register a [`WriteBack`] component.
    ///
    /// Adds write-back systems to both [`PreUpdate`] and [`PostUpdate`] (in
    /// the [`WriteBackSet`]). The `PreUpdate` pass flushes component-side
    /// changes into attributes before `Update`; the `PostUpdate` pass catches
    /// changes made during `Update`.
    fn register_write_back<T: WriteBack>(&mut self) -> &mut Self;
}

impl AttributesAppExt for App {
    fn register_attribute_derived<T: AttributeDerived>(&mut self) -> &mut Self {
        self.add_systems(
            PreUpdate,
            update_attribute_derived::<T>.in_set(AttributeDerivedSet),
        )
        .add_systems(
            PostUpdate,
            update_attribute_derived::<T>.in_set(AttributeDerivedSet),
        )
    }

    fn register_write_back<T: WriteBack>(&mut self) -> &mut Self {
        self.add_systems(
            PreUpdate,
            update_write_back::<T>.in_set(WriteBackSet),
        )
        .add_systems(
            PostUpdate,
            update_write_back::<T>.in_set(WriteBackSet),
        )
    }
}

// ---------------------------------------------------------------------------
// Inventory-based auto-registration
// ---------------------------------------------------------------------------

/// A registration entry for the [`inventory`] crate.
///
/// Each entry carries a function pointer that registers systems with the
/// Bevy [`App`]. Entries are submitted at link time (via `inventory::submit!`)
/// and collected in [`AttributesPlugin::build`](crate::plugin::AttributesPlugin).
///
/// The [`attribute_component!`] macro emits these automatically. For manual
/// implementations, use the [`register_derived!`] or [`register_write_back!`]
/// convenience macros:
///
/// ```ignore
/// register_derived!(MyCustomDerived);
/// register_write_back!(MyCustomWriteBack);
/// ```
pub struct AttributeRegistration {
    pub register_fn: fn(&mut App),
}

inventory::collect!(AttributeRegistration);

/// Register a [`AttributeDerived`] component via the `inventory` auto-registration
/// system. Place this at module scope.
///
/// ```ignore
/// register_derived!(PlayerHealth);
/// ```
#[macro_export]
macro_rules! register_derived {
    ($ty:ty) => {
        $crate::_register_attribute!(attribute_derived, $ty);
    };
}

/// Register a [`WriteBack`] component via the `inventory` auto-registration
/// system. Place this at module scope.
///
/// ```ignore
/// register_write_back!(PlayerInput);
/// ```
#[macro_export]
macro_rules! register_write_back {
    ($ty:ty) => {
        $crate::_register_attribute!(write_back, $ty);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! _register_attribute {
    (attribute_derived, $ty:ty) => {
        ::inventory::submit! {
            $crate::derived::AttributeRegistration {
                register_fn: |app| {
                    use $crate::derived::AttributesAppExt;
                    app.register_attribute_derived::<$ty>();
                }
            }
        }
    };
    (write_back, $ty:ty) => {
        ::inventory::submit! {
            $crate::derived::AttributeRegistration {
                register_fn: |app| {
                    use $crate::derived::AttributesAppExt;
                    app.register_write_back::<$ty>();
                }
            }
        }
    };
}
