//! Renzora reactive binding layer.
//!
//! Single primitive for every cross-panel sync problem: attach a
//! [`Bound`] component to a UI widget entity, point it at a source in
//! the ECS, and the reactive layer handles the rest.
//!
//! # Flow
//!
//! 1. Panel spawns a widget entity with a [`Bound`] component.
//! 2. [`sync_bindings`] runs each frame, reads the source, fires a
//!    [`BindingChanged`] event only when the value actually changed.
//! 3. Widget-specific observer systems react to `BindingChanged` and
//!    update their visible components (text, colour, node width…).
//! 4. When the user edits a widget, the widget emits a
//!    [`CommitBinding`] event.
//! 5. [`apply_commits`] (exclusive) runs the binding's sink, mutating
//!    the ECS source. `Changed<T>` fires upstream; next tick
//!    `sync_bindings` propagates the new value to every other widget
//!    bound to the same source — automatic cross-panel sync.
//!
//! # Why this shape
//!
//! - ECS is the single source of truth. Widgets never cache values
//!   (beyond the `last` field the sync loop owns).
//! - No panel needs to know any other panel exists. An inspector edit
//!   and a hierarchy rename touch the same `Name` component; both
//!   panels refresh from it independently.
//! - Granularity is per-binding: a change to `Transform.translation`
//!   doesn't wake widgets bound to `Transform.rotation`.

mod binding;
mod helpers;
mod plugin;
mod source;
mod value;

#[cfg(test)]
mod tests;

pub use binding::{apply_commits, sync_bindings, BindingChanged, Bound, CommitBinding, WidgetKind};
pub use plugin::ReactivePlugin;
pub use source::{BindSink, BindSource, SelectionProvider};
pub use value::BoundValue;
