//! `renzora::bus`: named events and services, so plugins that cannot share
//! Rust types can still talk to each other.
//!
//! Two marketplace plugins run in one `World`, but each was compiled on its own,
//! so a type one defines is a type the other has never heard of. What both can
//! name is the contract, so the bus speaks only contract types: a string for
//! *what* (`"combat.damaged"`, `"inventory.add_item"`) and a [`BusValue`] for
//! the data. Neither plugin needs the other installed to compile, load or run.
//!
//! ```ignore
//! use renzora::bus::prelude::*;
//!
//! // combat plugin
//! app.declare_event(
//!     EventDecl::new("combat.damaged")
//!         .field("entity", BusKind::Entity)
//!         .field("amount", BusKind::Float),
//! );
//! world.emit("combat.damaged", BusValue::map().with("entity", e).with("amount", 25.0));
//!
//! // hud plugin, which never links the combat plugin
//! app.on_event("combat.damaged", |world, ev| {
//!     let amount: f32 = ev.field("amount")?;
//!     Ok(())
//! });
//!
//! // inventory plugin provides, anyone calls
//! app.provide(
//!     ServiceDecl::new("inventory.add_item").arg("id", BusKind::String),
//!     |world, args| { let id: String = args.field("id")?; Ok(true.into()) },
//! );
//! let added = world.call("inventory.add_item", BusValue::map().with("id", "sword"));
//! ```
//!
//! **Delivery is immediate.** `World::emit` runs every listener before it
//! returns, and `Commands::emit` does the same when the system's commands are
//! applied. A queue drained by a system would need some crate to own that
//! system and pick its place in the schedule; immediate delivery needs no
//! installation at all, and nested emits are bounded by [`MAX_DEPTH`].
//!
//! **Name things `<plugin>.<thing>`.** The bus has one flat namespace for every
//! installed plugin, so the prefix is what keeps two plugins' `"damaged"` apart.
//! Change a payload incompatibly and give it a new name (`inventory.add_item@2`)
//! rather than breaking every caller of the old one.
//!
//! This is not Bevy's `Event` or `Message`: those are keyed by a Rust type,
//! which is exactly what two separately built plugins cannot agree on.

mod bus;
mod decl;
mod value;

pub use bus::{Bus, BusAppExt, BusCommandsExt, BusWorldExt, ListenerId, MAX_DEPTH};
pub use decl::{BusField, EventDecl, ServiceDecl};
pub use value::{BusKind, BusValue, FromBusValue};

/// Why a call failed.
#[derive(Clone, Debug, PartialEq)]
pub enum BusError {
    /// Nothing answers this name: usually, the plugin that would is not
    /// installed.
    NotProvided(String),
    /// The arguments did not match the declaration, or a handler's
    /// [`BusValue::field`] refused one.
    BadArgs(String),
    /// The handler ran and reported a failure of its own.
    Failed(String),
    /// Events and calls were nested [`MAX_DEPTH`] deep, almost always two
    /// plugins answering each other in a loop.
    TooDeep(String),
}

impl BusError {
    /// For a handler: `return Err(BusError::failed("no such item"))`.
    pub fn failed(message: impl Into<String>) -> Self {
        BusError::Failed(message.into())
    }
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BusError::NotProvided(name) => write!(f, "nothing provides `{name}`"),
            BusError::BadArgs(problem) => write!(f, "bad arguments: {problem}"),
            BusError::Failed(problem) => f.write_str(problem),
            BusError::TooDeep(name) => {
                write!(f, "`{name}` refused: events and calls nested {MAX_DEPTH} deep")
            }
        }
    }
}

impl std::error::Error for BusError {}

/// Everything a plugin needs, including the traits that put `emit`, `call`,
/// `provide` and `on_event` on `App`, `World` and `Commands`.
pub mod prelude {
    pub use crate::{
        BusAppExt, BusCommandsExt, BusError, BusKind, BusValue, BusWorldExt, EventDecl,
        ServiceDecl,
    };
}

#[cfg(test)]
mod tests;
