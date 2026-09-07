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

/// Per-entity override of the alpha mode a resolved [`MaterialRef`] renders
/// with.
///
/// A `.material` file declares one alpha mode for every mesh that wears it,
/// and for a mesh that is right: transparency is part of the look the author
/// signed off on. For an *overlay* it is wrong. A terrain paint layer wears
/// the same rock or water material a cliff or a lake does, but its edges have
/// to fade out through the per-vertex coverage alpha the overlay mesh carries,
/// whatever the material itself says about transparency. Without this the only
/// options are editing the material (which changes it everywhere it is used)
/// or keeping a near-duplicate `.material` per overlay.
///
/// The resolver honours it by cloning the resolved asset, never by touching
/// the cached master, so a second alpha mode costs one asset and no recompile.
#[derive(bevy::prelude::Component, Clone, Copy, Debug, PartialEq)]
pub struct MaterialAlphaOverride {
    pub mode: super::components::PbrAlphaMode,
    /// Discard threshold for [`super::components::PbrAlphaMode::Mask`].
    /// Ignored in the other modes.
    pub cutoff: f32,
}

impl MaterialAlphaOverride {
    /// Standard alpha blending — what an overlay wants, so its coverage
    /// feather is actually drawn.
    pub const BLEND: Self = Self {
        mode: super::components::PbrAlphaMode::Blend,
        cutoff: 0.5,
    };

    /// Hashable identity, for keying a cache of derived material variants.
    /// `f32` is not `Hash`/`Eq`, so the cutoff travels as its bit pattern.
    pub fn key(&self) -> (u8, u32) {
        use super::components::PbrAlphaMode;
        let mode = match self.mode {
            PbrAlphaMode::Opaque => 0,
            PbrAlphaMode::Mask => 1,
            PbrAlphaMode::Blend => 2,
        };
        (mode, self.cutoff.to_bits())
    }
}
