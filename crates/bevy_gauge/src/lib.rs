pub mod attribute_id;
pub mod attributes;
pub mod attributes_mut;
pub mod context;
pub mod derived;
pub mod expr;
pub mod graph;
pub mod instant;
pub mod modifier;
pub mod modifier_set;
pub mod node;
pub mod plugin;
pub mod requirements;
pub mod tags;

#[doc(hidden)]
pub mod macros;

// Re-export proc macros at crate root for reliable resolution in dependents
pub use bevy_gauge_macros::AttributeComponent;
pub use bevy_gauge_macros::define_tags;

pub mod prelude {
    pub use crate::attributes;
    pub use crate::attributes::Attributes;
    pub use crate::attributes_mut::AttributesMut;
    pub use crate::derived::{
        AttributeDerived, AttributeDerivedSet, AttributesAppExt, WriteBack, WriteBackSet,
    };
    pub use crate::expr::{CompileError, Expr};
    pub use crate::instant;
    pub use crate::instant::{
        AttributeQueries, EvaluatedInstantEntry, InstantExt, InstantModifierSet,
    };
    pub use crate::mod_set;
    pub use crate::modifier::Modifier;
    pub use crate::modifier_set::{AttributeInitializer, ModifierSet, ModifierValue};
    pub use crate::node::ReduceFn;
    pub use crate::plugin::AttributesPlugin;
    pub use crate::register_derived;
    pub use crate::register_write_back;
    pub use crate::requirements::AttributeRequirements;
    pub use crate::requires;
    pub use crate::tags::{TagMask, TagResolver};
    pub use bevy_gauge_macros::AttributeComponent;
    pub use bevy_gauge_macros::define_tags;
}
