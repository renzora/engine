pub mod attribute_id;
pub mod expr;
pub mod context;
pub mod modifier;
pub mod node;
pub mod tags;
pub mod graph;
pub mod attributes;
pub mod attributes_mut;
pub mod modifier_set;
pub mod derived;
pub mod instant;
pub mod requirements;
pub mod plugin;

#[doc(hidden)]
pub mod macros;

// Re-export proc macros at crate root for reliable resolution in dependents
pub use bevy_gauge_macros::AttributeComponent;
pub use bevy_gauge_macros::define_tags;

pub mod prelude {
    pub use crate::expr::{Expr, CompileError};
    pub use crate::modifier::Modifier;
    pub use crate::modifier_set::{ModifierSet, ModifierValue, AttributeInitializer};
    pub use crate::node::ReduceFn;
    pub use crate::tags::{TagMask, TagResolver};
    pub use crate::attributes::Attributes;
    pub use crate::attributes_mut::AttributesMut;
    pub use crate::derived::{
        AttributeDerived, WriteBack,
        AttributeDerivedSet, WriteBackSet, AttributesAppExt,
    };
    pub use crate::instant::{
        InstantModifierSet, EvaluatedInstantEntry,
        AttributeQueries, InstantExt,
    };
    pub use crate::requirements::AttributeRequirements;
    pub use crate::plugin::AttributesPlugin;
    pub use crate::attributes;
    pub use crate::mod_set;
    pub use crate::instant;
    pub use crate::requires;
    pub use crate::register_derived;
    pub use crate::register_write_back;
    pub use bevy_gauge_macros::AttributeComponent;
    pub use bevy_gauge_macros::define_tags;
}
