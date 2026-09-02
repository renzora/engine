//! Who owns the sky this frame.
//!
//! Two systems can paint the background: `renzora_atmosphere` (Bevy's physical
//! `Atmosphere`, a full-screen scattering pass) and `renzora_skybox` (a cubemap
//! `Skybox`). They are not composable — the atmosphere's `render_sky` runs
//! *after* the opaque pass, so after the skybox has drawn, and composites
//! `framebuffer * transmittance + inscattering` over it. A cubemap sky ends up
//! washed out to a pale haze, which is exactly what "I added a Skybox with an
//! HDR and nothing happened" looks like.
//!
//! So the two arbitrate through this resource instead of both drawing. It lives
//! in the contract crate because the writer (`renzora_skybox`) and the readers
//! (`renzora_atmosphere`, `renzora_engine`'s viewport sky sharing) are separate
//! crates that must agree on one `TypeId`.
//!
//! This is a stopgap until `WorldEnvironment` grows its `background` section
//! (see `docs/world-environment-spec.md`), at which point the choice becomes a
//! single authored enum and the arbitration disappears.

use bevy::prelude::*;

/// Set by `renzora_skybox` while a `SkyboxData` component is driving the sky.
///
/// When active:
/// - `renzora_atmosphere` scales the planet's scattering density to
///   [`Self::atmosphere_blend`], so the scattering pass thins out and the
///   cubemap shows through. The `Atmosphere` component is never removed —
///   removing it restructures the mesh-view bind group and crashes wgpu.
/// - `renzora_engine::camera::share_sky_to_secondary_viewports` stands down,
///   because the skybox already attaches its own `Skybox` to *every* camera.
///   Without this both write `Skybox` every frame and fight over the handle.
/// - The atmosphere's environment bake is therefore black, so the skybox also
///   takes over image-based lighting (it feeds its own cubemap into
///   `GeneratedEnvironmentMapLight`). Sky and IBL have to move together or the
///   scene loses all ambient light.
#[derive(Resource, Debug, Clone, Copy)]
pub struct SkyTakeover {
    pub active: bool,
    /// The sky's own brightness multiplier (a panorama's `energy`), so the light
    /// it casts tracks the sky you can see. `renzora_environment_map` folds this
    /// into the IBL intensity rather than the skybox baking it into the cubemap,
    /// because baking it would mean regenerating that cubemap on every frame of
    /// a slider drag.
    pub energy: f32,
    /// How much of the atmosphere's scattering still applies, 0..1, while the
    /// skybox owns the sky.
    ///
    /// Not a binary off, because `render_sky` composites
    /// `framebuffer * transmittance + inscattering` over the **whole frame** —
    /// geometry included. Switching it off entirely takes the aerial
    /// perspective with it, and the scene loses the sun-centred glow and
    /// distance haze that made it look lit. A partial density keeps that
    /// coupling (the sky and the ground both pick up the sun's colour as it
    /// sets) while leaving the cubemap readable underneath, which full density
    /// does not.
    pub atmosphere_blend: f32,
    /// A **photographed** sky is drawing, so the procedural cloud deck stands
    /// down (`renzora_clouds` treats this exactly like its `enabled` toggle
    /// being off, dome and all).
    ///
    /// Narrower than [`Self::active`] on purpose. The atmosphere is a
    /// *background*, and any skybox mode replaces it. Clouds are a *layer* in
    /// front of the sky, and a gradient, colour, or tiled backdrop with a
    /// volumetric deck in front of it is a perfectly good sky — that's the stock
    /// look. An HDR panorama already has its own clouds baked in, so a second
    /// deck on top of it just fogs the photo.
    pub suppress_clouds: bool,
}

impl Default for SkyTakeover {
    fn default() -> Self {
        Self {
            active: false,
            energy: 1.0,
            atmosphere_blend: 0.0,
            suppress_clouds: false,
        }
    }
}
