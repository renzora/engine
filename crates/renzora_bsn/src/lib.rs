//! Interim BSN scene (de)serializer for Renzora on Bevy 0.19.
//!
//! ## Why this crate exists
//!
//! Bevy 0.19's BSN rework **deleted** the runtime scene-serialization API that
//! Renzora's save/load (`renzora_engine::scene_io`) was built on:
//! `DynamicScene`, `DynamicSceneBuilder`, `bevy::scene::serde::SceneDeserializer`,
//! `DynamicEntity`, and `DynamicScene::write_to_world`. The replacement — BSN —
//! is **compile-time only** in 0.19 (`bsn!` is a macro; there is no runtime BSN
//! text parser yet; see bevy#23576). So Renzora must own the format in the
//! interim.
//!
//! ## What this crate provides
//!
//! 1. A **reflection-based scene IR** — [`DynamicScene`] / [`DynamicEntity`] /
//!    [`DynamicSceneBuilder`] / [`SceneFilter`] — ported from `bevy_scene 0.18`
//!    to 0.19 reflection. This is the *extraction* layer: it lets `scene_io.rs`
//!    keep its mature deny-lists and per-component filtering unchanged.
//! 2. A **BSN-flavored text format** ([`bsn`]) behind the [`SceneSerializer`]
//!    trait: entities are emitted as BSN-style blocks with their children nested
//!    in `[ … ]`, and each component value is encoded with `bevy_reflect`'s serde
//!    (the reliable part). When bevy ships a first-party runtime BSN loader, the
//!    [`SceneSerializer`] impl is the single thing that swaps out.
//!
//! The on-disk format is **interim** and owned by Renzora — it is BSN-shaped
//! (entity blocks + bracketed children) but not yet byte-identical to upstream
//! BSN value syntax. Existing RON `.scene` files predate it and need converting.

mod dynamic_scene;
mod dynamic_scene_builder;
mod reflect_utils;
mod scene_filter;

pub mod bsn;

pub use bsn::register_component_alias;
pub use dynamic_scene::{DynamicEntity, DynamicScene};
pub use dynamic_scene_builder::DynamicSceneBuilder;
pub use scene_filter::SceneFilter;

/// Errors that can occur when writing a [`DynamicScene`] back into a `World`.
///
/// Mirrors the subset of `bevy_scene 0.18`'s `SceneSpawnError` that the
/// reflection write-path can raise.
#[derive(thiserror::Error, Debug)]
pub enum SceneSpawnError {
    /// A reflected value carried no represented type info (an opaque dynamic).
    #[error("scene contains a value with no represented type: {type_path}")]
    NoRepresentedType {
        /// The reflect type path of the offending value.
        type_path: String,
    },
    /// The type is reflected but not registered in the `AppTypeRegistry`.
    #[error("scene contains the unregistered type `{type_path}`; register it with `app.register_type`")]
    UnregisteredButReflectedType {
        /// The type path of the unregistered type.
        type_path: String,
    },
    /// The type is registered but is missing `ReflectComponent` type data.
    #[error("scene contains the unregistered component `{type_path}`; add `#[reflect(Component)]`")]
    UnregisteredComponent {
        /// The component type path.
        type_path: String,
    },
    /// The type is registered but is missing `ReflectResource` type data.
    #[error("scene contains the unregistered resource `{type_path}`; add `#[reflect(Resource)]`")]
    UnregisteredResource {
        /// The resource type path.
        type_path: String,
    },
}
