//! Pointing an entity at a `.material` file, and saying when that pointer has
//! been resolved.

// For the `ReflectComponent` / `ReflectSerialize` / `ReflectDeserialize` types
// the `#[reflect(..)]` attribute below expands to by bare name.
use bevy::prelude::*;

/// Reference to a material file. Add to any entity with `Mesh3d` to assign a material.
#[derive(
    bevy::prelude::Component,
    serde::Serialize,
    serde::Deserialize,
    bevy::prelude::Reflect,
    Clone,
    Debug,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MaterialRef(pub String);

/// Marker added by the material resolver once a [`MaterialRef`] has been loaded,
/// compiled and attached. Removing it is how *any* crate says "this entity's
/// material changed, resolve it again" — which is why the marker lives here and
/// not in `renzora_shader`: the editor panels that rebind a material (the
/// hierarchy's drag-to-attach, the material inspector, the viewport drop) would
/// otherwise each have to link the whole shader crate for one component.
#[derive(bevy::prelude::Component)]
pub struct MaterialResolved {
    /// The `MaterialRef` path this entity was resolved from.
    pub source_path: String,
}
