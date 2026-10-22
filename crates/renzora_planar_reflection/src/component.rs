//! What a scene saves: the [`PlanarReflection`] component and the one global
//! knob, [`PlanarReflectionSettings`].
//!
//! Always compiled, whatever the renderer features, so a scene carrying a
//! mirror deserializes on every build. A build without the 3D renderer simply
//! draws the surface with its own material.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Which local axis of the entity is the reflecting surface's normal.
///
/// The plane always passes through the entity's origin. `Y` is right for the
/// engine's own Plane primitive in any orientation (floors, water, puddles, and
/// the Mirror preset, which is that plane stood upright); `Z` is right for a
/// mesh modelled facing +Z, as most imported mirror and window meshes are.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize,
)]
#[reflect(Serialize, Deserialize)]
pub enum NormalAxis {
    X,
    #[default]
    Y,
    Z,
}

impl NormalAxis {
    pub const LABELS: [&'static str; 3] = ["X", "Y", "Z"];

    pub fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    /// Parse a label from [`Self::label`]; anything else is the default.
    pub fn from_label(label: &str) -> Self {
        match label {
            "X" => Self::X,
            "Z" => Self::Z,
            _ => Self::Y,
        }
    }

    /// The axis in the entity's local space.
    pub fn local(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }
}

/// A flat surface that reflects the scene, the engine's equivalent of Unreal's
/// Planar Reflection actor.
///
/// Put it on an entity with a flat mesh. For every camera that can see the
/// surface (up to [`PlanarReflectionSettings::max_active`] at once), the
/// engine renders the scene a second time from that camera mirrored across the
/// plane, and draws the result over the surface's own material. Unlike SSR it
/// shows what is off screen, and unlike SSR and Solari it shows particles and
/// transparent objects, because it is an ordinary render of the scene.
///
/// The surface is one-sided: it reflects toward its normal, and a camera behind
/// it sees only the material.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PlanarReflection {
    /// Off keeps the component (and its settings) but renders nothing.
    pub enabled: bool,
    /// Which local axis is the surface normal. See [`NormalAxis`].
    pub normal_axis: NormalAxis,
    /// Capture resolution as a fraction of the viewing camera's, 0.25 to 1.
    /// The capture costs a scene render at this size, so it is the main
    /// performance knob; 0.5 is a quarter of the pixels.
    pub resolution_scale: f32,
    /// How much of the reflection is laid over the material, 0 to 1.
    pub strength: f32,
    /// Multiplied into the reflection: a dark tint for smoked glass, a
    /// coloured one for metal.
    pub tint: Color,
    /// How much more it reflects at grazing angles, 0 to 1. 0 reflects the
    /// same amount from every angle (a mirror); 1 follows the Schlick curve of
    /// water or glass, nearly transparent head on and a full mirror at grazing.
    pub fresnel: f32,
    /// Softens the reflection for frosted, wet or rough surfaces, 0 to 1.
    pub blur: f32,
    /// How far in front of the surface the capture starts clipping, in world
    /// units. Geometry closer to the plane than this is cut from the
    /// reflection, which is what stops the surface reflecting itself (acne,
    /// shimmer); objects actually touching the mirror lose that sliver.
    pub clip_offset: f32,
    /// Beyond this distance from the camera the surface stops updating and
    /// shows only its material. 0 means no limit.
    pub max_distance: f32,
    /// Re-render every N frames. 1 is every frame; 2 halves the cost and the
    /// reflection lags a frame behind fast motion.
    pub update_every: u32,
    /// Put the plane through the middle of the mesh's bounds rather than the
    /// entity's origin.
    ///
    /// On by default because the two are rarely the same for imported meshes:
    /// a window from a merged model keeps its vertices where they sat in the
    /// building and its origin at the model's origin, so a plane through the
    /// origin can be metres from the glass, and the reflection then showed
    /// the scene mirrored about the wrong place. For a flat mesh the bounds'
    /// middle is on the surface whichever way it is offset.
    pub fit_to_mesh: bool,
    /// Moves the plane along its normal, in world units, after fitting. For a
    /// mesh whose reflecting face is not its middle (a slab of glass, a
    /// recessed puddle).
    pub plane_offset: f32,
    /// Reflect toward the other side of the chosen axis: for a mesh whose
    /// local axis points into the surface rather than out of it.
    pub flip: bool,
}

impl Default for PlanarReflection {
    fn default() -> Self {
        Self {
            enabled: true,
            normal_axis: NormalAxis::Y,
            resolution_scale: 0.5,
            strength: 1.0,
            tint: Color::WHITE,
            fresnel: 0.5,
            blur: 0.0,
            clip_offset: 0.02,
            max_distance: 100.0,
            update_every: 1,
            fit_to_mesh: true,
            plane_offset: 0.0,
            flip: false,
        }
    }
}

impl PlanarReflection {
    /// A wall mirror: reflects fully from every angle.
    pub fn mirror(normal_axis: NormalAxis) -> Self {
        Self {
            normal_axis,
            strength: 1.0,
            fresnel: 0.0,
            ..default()
        }
    }

    /// [`Self::resolution_scale`] clamped to its documented range. Values
    /// come straight from a scene file or an inspector drag, so every reader
    /// goes through these rather than trusting the field.
    pub fn resolution_scale(&self) -> f32 {
        finite_or(self.resolution_scale, 0.5).clamp(0.25, 1.0)
    }

    pub fn strength(&self) -> f32 {
        finite_or(self.strength, 1.0).clamp(0.0, 1.0)
    }

    pub fn fresnel(&self) -> f32 {
        finite_or(self.fresnel, 0.0).clamp(0.0, 1.0)
    }

    pub fn blur(&self) -> f32 {
        finite_or(self.blur, 0.0).clamp(0.0, 1.0)
    }

    pub fn clip_offset(&self) -> f32 {
        finite_or(self.clip_offset, 0.02).max(0.0)
    }

    /// `None` when unlimited.
    pub fn max_distance(&self) -> Option<f32> {
        let d = finite_or(self.max_distance, 0.0);
        (d > 0.0).then_some(d)
    }

    pub fn update_every(&self) -> u32 {
        self.update_every.max(1)
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

/// Global planar-reflection settings.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PlanarReflectionSettings {
    /// How many (surface, camera) pairs may render at once. Each is a full
    /// extra scene render, so this bounds the worst case; the nearest surfaces
    /// win and the rest show their material until they are among the nearest.
    pub max_active: usize,
}

impl Default for PlanarReflectionSettings {
    fn default() -> Self {
        Self { max_active: 4 }
    }
}
